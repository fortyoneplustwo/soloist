use serde::Deserialize;

#[derive(Deserialize)]
pub struct Device {
    pub id: String,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct PlaybackState {
    pub device: Device,
    pub is_playing: bool,
}

#[derive(Deserialize)]
pub struct AvailableDevices {
    pub devices: Vec<Device>,
}
