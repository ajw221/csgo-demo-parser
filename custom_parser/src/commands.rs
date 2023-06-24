#[derive(Debug)]
pub enum PacketCommand {
    Signon = 1,
    Packet = 2,
    Sync = 3,
    Console = 4,
    User = 5,
    Data = 6,
    Stop = 7,
    Custom = 8,
    String = 9,
}

impl From<u8> for PacketCommand {
    fn from(cmd: u8) -> Self {
        match cmd {
            1 => PacketCommand::Signon,
            2 => PacketCommand::Packet,
            3 => PacketCommand::Sync,
            4 => PacketCommand::Console,
            5 => PacketCommand::User,
            6 => PacketCommand::Data,
            7 => PacketCommand::Stop,
            8 => PacketCommand::Custom,
            9 => PacketCommand::String,
            _ => panic!("cmd: {cmd}"),
        }
    }
}

#[derive(Debug)]
pub enum MessageType {
    Tick = 4,
    SignonState = 7,
    ClassInfo = 10,
    VoiceInit = 14,
    VoiceData = 15,
    Sounds = 17,
    SetView = 18,
    TempEntities = 27,
    Prefetch = 28,
    PlayerAvatarData = 100,

    SetConVar = 6,
    ServerInfo = 8,
    CreateStringTable = 12,
    UpdateStringTable = 13,
    GameEvent = 25,
    PacketEntities = 26,
    GameEventList = 30,

    None,
}

impl From<u32> for MessageType {
    fn from(cmd: u32) -> Self {
        match cmd {
            4 => MessageType::Tick,
            7 => MessageType::SignonState,
            10 => MessageType::ClassInfo,
            14 => MessageType::VoiceInit,
            15 => MessageType::VoiceData,
            17 => MessageType::Sounds,
            18 => MessageType::SetView,
            27 => MessageType::TempEntities,
            28 => MessageType::Prefetch,
            100 => MessageType::PlayerAvatarData,

            6 => MessageType::SetConVar,
            8 => MessageType::ServerInfo,
            12 => MessageType::CreateStringTable,
            13 => MessageType::UpdateStringTable,
            25 => MessageType::GameEvent,
            26 => MessageType::PacketEntities,
            30 => MessageType::GameEventList,
            _ => MessageType::None,
        }
    }
}

impl MessageType {
    pub fn is_skippable(&self) -> bool {
        matches!(
            self,
            MessageType::Tick
                | MessageType::SignonState
                | MessageType::ClassInfo
                | MessageType::VoiceInit
                | MessageType::VoiceData
                | MessageType::Sounds
                | MessageType::SetView
                | MessageType::TempEntities
                | MessageType::Prefetch
                | MessageType::PlayerAvatarData
        )
    }
}
