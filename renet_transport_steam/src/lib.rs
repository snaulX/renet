use std::{
    error::Error,
    fmt, io,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
};

use renet::Transport;
use steamworks::{networking_messages::NetworkingMessages, Client, ClientManager, SteamId, networking_types::{NetworkingIdentity, SendFlags}};

pub struct SteamTransport {
    networking_messages: NetworkingMessages<ClientManager>,
}

const CHANNEL_ID: u32 = 0;

#[derive(Debug)]
pub enum SteamTransportError {
    InvalidAddress,
}

impl Error for SteamTransportError {}

impl fmt::Display for SteamTransportError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use SteamTransportError::*;

        match *self {
            InvalidAddress => write!(fmt, "received invalid address to send to"),
        }
    }
}

pub fn address_from_steam_id(steam_id: SteamId) -> SocketAddr {
    let raw = steam_id.raw();
    let segments: [u8; 8] = raw.to_le_bytes();
    let segments: [u16; 8] = segments.map(u16::from);

    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(segments), 0, 0, 0))
}

fn steam_id_from_address(address: SocketAddr) -> Result<SteamId, SteamTransportError> {
    if let SocketAddr::V6(address) = address {
        let ip = address.ip();
        let segments = ip.segments();
        let segments: [u8; 8] = segments.map(|x| x as u8);
        let raw = u64::from_le_bytes(segments);
        return Ok(SteamId::from_raw(raw));
    }

    Err(SteamTransportError::InvalidAddress)
}

impl SteamTransport {
    pub fn new(client: &Client<ClientManager>) -> Self {
        let networking_messages = client.networking_messages();

        Self {
            networking_messages
        }
    }
}

impl Transport for SteamTransport {
    fn recv_from(&mut self, buffer: &mut [u8]) -> Result<Option<(usize, SocketAddr)>, Box<dyn Error + Send + Sync + 'static>> {
        let messages = self.networking_messages.receive_messages_on_channel(CHANNEL_ID, 1);

        if let Some(message) = messages.get(0) {
            let network_id = message.identity_peer();
            let addr = match network_id.steam_id() {
                Some(steam_id) => address_from_steam_id(steam_id),
                None => {
                    log::warn!("Received message without steam id");
                    return Ok(None);
                }
            };

            let data = message.data();
            if data.len() > buffer.len() {
                log::error!(
                    "Received message bigger than buffer, got {}, expected less than {}",
                    data.len(),
                    buffer.len()
                );

                return Ok(None);
            }

            buffer[..data.len()].copy_from_slice(data);
            return Ok(Some((data.len(), addr)));
        }

        Ok(None)
    }

    fn send_to(&mut self, buffer: &[u8], addr: SocketAddr) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let steam_id = steam_id_from_address(addr)?;
        let network_id = NetworkingIdentity::new_steam_id(steam_id);
        self.networking_messages.send_message_to_user(network_id, SendFlags::UNRELIABLE_NO_NAGLE, buffer, CHANNEL_ID);

        Ok(())
    }
}