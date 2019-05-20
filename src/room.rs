//! Room state
use std::{
    cmp::{Ordering, PartialOrd},
    collections::{hash_map::Entry, HashMap},
    fmt,
};

use log::debug;
use ndarray::{Array, Ix2};
use screeps_api::{
    endpoints::room_terrain::TerrainType,
    websocket::{
        types::room::flags::Flag, types::room::objects::KnownRoomObject, RoomUpdate, RoomUserInfo,
    },
    RoomName, RoomTerrain,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomId {
    pub shard: Option<String>,
    pub room_name: RoomName,
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.shard {
            Some(s) => write!(f, "{}:{}", s, self.room_name),
            None => write!(f, "{}", self.room_name),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, smart_default::SmartDefault, derive_more::Display)]
pub enum ConnectionState {
    #[default]
    #[display(fmt = "disconnected")]
    Disconnected,
    #[display(fmt = "authenticating...")]
    Authenticating,
    #[display(fmt = "connected")]
    Connected,
    #[display(fmt = "error!")]
    Error,
}

impl RoomId {
    pub fn new(shard: Option<String>, room_name: RoomName) -> Self {
        RoomId { shard, room_name }
    }
}

#[derive(Clone, Debug)]
pub struct Room {
    last_update_time: Option<u32>,
    room: RoomId,
    terrain: RoomTerrain,
    objects: HashMap<String, KnownRoomObject>,
    flags: Vec<Flag>,
    users: HashMap<String, RoomUserInfo>,
}

impl Room {
    pub fn new(room: RoomId, terrain: RoomTerrain) -> Self {
        assert_eq!(room.room_name, terrain.room_name);
        Room {
            last_update_time: None,
            room,
            terrain,
            objects: HashMap::new(),
            flags: Vec::new(),
            users: HashMap::new(),
        }
    }

    pub fn update(&mut self, update: RoomUpdate) -> Result<(), serde_json::Error> {
        debug!("updating metadata");
        if let Some(time) = update.game_time {
            self.last_update_time = Some(time);
        }
        debug!("updating objects");
        for (id, data) in update.objects.into_iter() {
            debug!(
                "updating {} with data:\n\t{}",
                id,
                serde_json::to_string_pretty(&data).unwrap()
            );
            if data.is_null() {
                self.objects.remove(&id);
            } else {
                match self.objects.entry(id) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update(data)?;
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(serde_json::from_value(data)?);
                    }
                }
            }
        }
        debug!("updating flags");
        self.flags = update.flags;

        debug!("updating users");
        for (user_id, data) in update.users.into_iter().flat_map(|x| x) {
            debug!(
                "updating user {} with data:\n\t{}",
                user_id,
                serde_json::to_string_pretty(&data).unwrap()
            );
            if data.is_null() {
                self.users.remove(&user_id);
            } else {
                match self.users.entry(user_id) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().update(serde_json::from_value(data)?);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(serde_json::from_value(data)?);
                    }
                }
            }
        }

        debug!("update complete");

        Ok(())
    }

    pub fn visualize(&self) -> VisualRoom {
        let mut room = VisualRoom::new(self.last_update_time, self.room.clone());

        for (row_idx, row) in self.terrain.terrain.iter().enumerate() {
            for (col_idx, item) in row.iter().enumerate() {
                if let Some(itt) = InterestingTerrainType::from_terrain(*item) {
                    room.push_top(VisualObject::InterestingTerrain {
                        x: col_idx as u32,
                        y: row_idx as u32,
                        ty: itt,
                    });
                }
            }
        }

        for flag in &self.flags {
            room.push_top(VisualObject::Flag(flag.clone()));
        }

        for obj in self.objects.values() {
            room.push_top(VisualObject::RoomObject(obj.clone()));
        }

        for list in room.objs.iter_mut() {
            list.sort_unstable();
        }

        room
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum RoomObjectType {
    Road,
    Container,
    Tombstone,
    Resource,
    Rampart,
    Wall,
    Source,
    Mineral,
    KeeperLair,
    Controller,
    Extractor,
    Extension,
    Spawn,
    Portal,
    Link,
    Storage,
    Tower,
    Observer,
    PowerBank,
    PowerSpawn,
    Lab,
    Terminal,
    Nuker,
    Creep,
}

impl RoomObjectType {
    pub fn of(obj: &KnownRoomObject) -> Self {
        macro_rules! transformit {
            ( $($id:ident),* $(,)? ) => {
                match obj {
                    $(
                        KnownRoomObject::$id(_) => RoomObjectType::$id,
                    )*
                }
            };
        }
        transformit!(
            Road, Container, Tombstone, Resource, Rampart, Wall, Source, Mineral, KeeperLair,
            Controller, Extractor, Extension, Spawn, Portal, Link, Storage, Tower, Observer,
            PowerBank, PowerSpawn, Lab, Terminal, Nuker, Creep,
        )
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum InterestingTerrainType {
    Swamp,
    Wall,
}

impl InterestingTerrainType {
    pub fn from_terrain(terrain: TerrainType) -> Option<Self> {
        match terrain {
            TerrainType::Plains => None,
            TerrainType::Swamp => Some(InterestingTerrainType::Swamp),
            TerrainType::Wall | TerrainType::SwampyWall => Some(InterestingTerrainType::Wall),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisualObject {
    InterestingTerrain {
        x: u32,
        y: u32,
        ty: InterestingTerrainType,
    },
    Flag(Flag),
    RoomObject(KnownRoomObject),
}

impl VisualObject {
    fn x(&self) -> u32 {
        match self {
            VisualObject::InterestingTerrain { x, .. } => *x,
            VisualObject::Flag(x) => x.x,
            VisualObject::RoomObject(x) => x.x(),
        }
    }

    fn y(&self) -> u32 {
        match self {
            VisualObject::InterestingTerrain { y, .. } => *y,
            VisualObject::Flag(x) => x.y,
            VisualObject::RoomObject(x) => x.y(),
        }
    }
}

impl PartialEq for VisualObject {
    fn eq(&self, other: &VisualObject) -> bool {
        use VisualObject::*;
        match (self, other) {
            (
                InterestingTerrain {
                    ty: ty1,
                    x: x1,
                    y: y1,
                },
                InterestingTerrain {
                    ty: ty2,
                    x: x2,
                    y: y2,
                },
            ) => ty1 == ty2 && x1 == x2 && y1 == y2,
            (Flag(a), Flag(b)) => a == b,
            (RoomObject(a), RoomObject(b)) => {
                RoomObjectType::of(a) == RoomObjectType::of(b) && a.id() == b.id()
            }
            (..) => false,
        }
    }
}

impl Eq for VisualObject {}

impl PartialOrd for VisualObject {
    fn partial_cmp(&self, other: &VisualObject) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VisualObject {
    fn cmp(&self, other: &VisualObject) -> Ordering {
        use VisualObject::*;
        match (self, other) {
            (
                InterestingTerrain {
                    ty: ty1,
                    x: x1,
                    y: y1,
                },
                InterestingTerrain {
                    ty: ty2,
                    x: x2,
                    y: y2,
                },
            ) => ty1.cmp(ty2).then(x1.cmp(x2)).then(y1.cmp(y2)),
            (InterestingTerrain { .. }, _) => Ordering::Less,
            (_, InterestingTerrain { .. }) => Ordering::Greater,
            (Flag(a), Flag(b)) => a.name.cmp(&b.name),
            (Flag(_), _) => Ordering::Less,
            (_, Flag(_)) => Ordering::Greater,
            (RoomObject(a), RoomObject(b)) => RoomObjectType::of(a)
                .cmp(&RoomObjectType::of(b))
                .then_with(|| a.id().cmp(b.id())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualRoom {
    pub last_update_time: Option<u32>,
    pub room_id: RoomId,
    pub objs: Array<Vec<VisualObject>, Ix2>,
}

impl VisualRoom {
    fn new(last_update_time: Option<u32>, room_id: RoomId) -> Self {
        VisualRoom {
            last_update_time,
            room_id,
            objs: Array::from_elem((50, 50), Vec::new()),
        }
    }
}

impl VisualRoom {
    fn push_top(&mut self, item: VisualObject) {
        self.objs
            .get_mut([item.x() as usize, item.y() as usize])
            .expect("expected all objects to have valid coordinates (0-49)")
            .push(item);
    }
}
