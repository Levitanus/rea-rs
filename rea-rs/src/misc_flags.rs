use bitflags::bitflags;

bitflags! {
    pub struct UndoFlags:u8{
        const TRACK_CONFIGURATIONS = 1;
        const TRACK_FX = 2;
        const TRACK_ITEMS = 4;
        const PROJECT_STATES = 8;
        const FREEZE_STATES = 16;
    }
}
