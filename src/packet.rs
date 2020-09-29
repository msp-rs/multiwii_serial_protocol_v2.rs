use crate::prelude::v1::*;
use crc_any::CRCu8;
use packed_struct::{PackedStructSlice, PackingError};

#[derive(Clone, Debug, PartialEq)]
pub enum MspError {
    UnknownDirection(u8),
    UnknownVersion(u8),
    CrcMismatch { expected: u8, calculated: u8 },
    OutputBufferSizeMismatch,
    Packing(PackingError),
}

/// Request: Master to Slave (`<`)
/// Response: Slave to Master (`>`)
/// Error: Master to Slave or Slave to Master (`!`)
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MspPacketDirection {
    Request,
    Response,
    Error,
}

impl From<MspPacketDirection> for u8 {
    fn from(d: MspPacketDirection) -> Self {
        match d {
            MspPacketDirection::Request => b'<',
            MspPacketDirection::Response => b'>',
            MspPacketDirection::Error => b'!',
        }
    }
}

impl TryFrom<u8> for MspPacketDirection {
    type Error = MspError;
    fn try_from(byte: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match byte as char {
            '<' => Ok(MspPacketDirection::Request),
            '>' => Ok(MspPacketDirection::Response),
            '!' => Ok(MspPacketDirection::Error),
            _ => Err(MspError::UnknownDirection(byte)),
        }
    }
}

/// V1: (`M`)
/// V2: (`X`)
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MspVersion {
    V1,
    V2,
}

impl From<&MspVersion> for u8 {
    fn from(d: &MspVersion) -> Self {
        match d {
            MspVersion::V1 => b'M',
            MspVersion::V2 => b'X',
        }
    }
}

impl TryFrom<u8> for MspVersion {
    type Error = MspError;
    fn try_from(byte: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match byte as char {
            'M' => Ok(MspVersion::V1),
            'X' => Ok(MspVersion::V2),
            _ => Err(MspError::UnknownVersion(byte)),
        }
    }
}

pub trait MspPayload: Sized + PartialEq {
    // We can not implement this yet, because we do not know which message maps to which ID
    //const ID: IdType;
    fn len(&self) -> usize;
    fn decode(r: &[u8]) -> Result<Self, MspError>
    where
        Self: std::marker::Sized;
    fn encode(&self) -> Result<Vec<u8>, MspError>;
}

impl<P> MspPayload for P
where
    P: PackedStructSlice + PartialEq,
{
    fn len(&self) -> usize {
        P::packed_bytes()
    }

    fn decode(r: &[u8]) -> Result<Self, MspError> {
        PackedStructSlice::unpack_from_slice(r).map_err(MspError::Packing)
    }

    fn encode(&self) -> Result<Vec<u8>, MspError> {
        self.pack_to_vec().map_err(MspError::Packing)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// A decoded MSP packet, with a command code, direction and payload
pub struct MspPacket<P: MspPayload + PartialEq + Sized> {
    pub cmd: u16,
    pub direction: MspPacketDirection,
    pub data: Option<P>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum MspParserState {
    Header1,
    Header2,
    Direction,
    FlagV2,
    DataLength,
    DataLengthV2,
    Command,
    CommandV2,
    Data,
    DataV2,
    Crc,
}

#[derive(Debug)]
/// Parser that can find packets from a raw byte stream
pub struct MspParser {
    state: MspParserState,
    packet_version: MspVersion,
    packet_direction: MspPacketDirection,
    packet_cmd: u16,
    packet_data_length_remaining: usize,
    packet_data: Vec<u8>,
    packet_crc: u8,
    packet_crc_v2: CRCu8,
}

impl MspParser {
    /// Create a new parser
    pub fn new() -> MspParser {
        Self {
            state: MspParserState::Header1,
            packet_version: MspVersion::V1,
            packet_direction: MspPacketDirection::Request,
            packet_data_length_remaining: 0,
            packet_cmd: 0,
            packet_data: Vec::new(),
            packet_crc: 0,
            packet_crc_v2: CRCu8::crc8dvb_s2(),
        }
    }

    /// Are we waiting for the header of a brand new packet?
    pub fn state_is_between_packets(&self) -> bool {
        self.state == MspParserState::Header1
    }

    /// Parse the next input byte. Returns a valid packet whenever a full packet is received, otherwise
    /// restarts the state of the parser.
    pub fn parse<P: MspPayload + PartialEq + Sized>(
        &mut self,
        input: u8,
    ) -> Result<Option<MspPacket<P>>, MspError> {
        match self.state {
            MspParserState::Header1 => {
                if input == b'$' {
                    self.state = MspParserState::Header2;
                } else {
                    self.reset();
                }
            }

            MspParserState::Header2 => {
                self.packet_version = input.try_into()?;
                self.state = MspParserState::Direction;
            }

            MspParserState::Direction => {
                self.packet_direction = input.try_into()?;

                self.state = match self.packet_version {
                    MspVersion::V1 => MspParserState::DataLength,
                    MspVersion::V2 => MspParserState::FlagV2,
                };
            }

            MspParserState::FlagV2 => {
                // uint8, flag, usage to be defined (set to zero)
                self.state = MspParserState::CommandV2;
                self.packet_data = Vec::with_capacity(2);
                self.packet_crc_v2.digest(&[input]);
            }

            MspParserState::CommandV2 => {
                self.packet_data.push(input);

                if self.packet_data.len() == 2 {
                    let mut s = [0u8; size_of::<u16>()];
                    s.copy_from_slice(&self.packet_data);
                    self.packet_cmd = u16::from_le_bytes(s);

                    self.packet_crc_v2.digest(&self.packet_data);
                    self.packet_data.clear();
                    self.state = MspParserState::DataLengthV2;
                }
            }

            MspParserState::DataLengthV2 => {
                self.packet_data.push(input);

                if self.packet_data.len() == 2 {
                    let mut s = [0u8; size_of::<u16>()];
                    s.copy_from_slice(&self.packet_data);
                    self.packet_data_length_remaining = u16::from_le_bytes(s).into();
                    self.packet_crc_v2.digest(&self.packet_data);
                    self.packet_data =
                        Vec::with_capacity(self.packet_data_length_remaining as usize);

                    if self.packet_data_length_remaining == 0 {
                        self.state = MspParserState::Crc;
                    } else {
                        self.state = MspParserState::DataV2;
                    }
                }
            }

            MspParserState::DataV2 => {
                self.packet_data.push(input);
                self.packet_data_length_remaining -= 1;

                if self.packet_data_length_remaining == 0 {
                    self.state = MspParserState::Crc;
                }
            }

            MspParserState::DataLength => {
                self.packet_data_length_remaining = input as usize;
                self.state = MspParserState::Command;
                self.packet_crc ^= input;
                self.packet_data = Vec::with_capacity(input as usize);
            }

            MspParserState::Command => {
                self.packet_cmd = input as u16;

                if self.packet_data_length_remaining == 0 {
                    self.state = MspParserState::Crc;
                } else {
                    self.state = MspParserState::Data;
                }

                self.packet_crc ^= input;
            }

            MspParserState::Data => {
                self.packet_data.push(input);
                self.packet_data_length_remaining -= 1;

                self.packet_crc ^= input;

                if self.packet_data_length_remaining == 0 {
                    self.state = MspParserState::Crc;
                }
            }

            MspParserState::Crc => {
                if self.packet_version == MspVersion::V2 {
                    self.packet_crc_v2.digest(&self.packet_data);
                    self.packet_crc = self.packet_crc_v2.get_crc();
                }

                let packet_crc = self.packet_crc;
                if input != packet_crc {
                    self.reset();
                    return Err(MspError::CrcMismatch {
                        expected: input,
                        calculated: packet_crc,
                    });
                }

                let mut n = Vec::new();
                mem::swap(&mut self.packet_data, &mut n);

                let payload = match n.len() == 0 {
                    true => None,
                    false => Some(P::decode(&n)?),
                };

                let packet = MspPacket {
                    cmd: self.packet_cmd,
                    direction: self.packet_direction,
                    data: payload,
                };

                self.reset();

                return Ok(Some(packet));
            }
        }

        Ok(None)
    }

    pub fn reset(&mut self) {
        self.state = MspParserState::Header1;
        self.packet_direction = MspPacketDirection::Request;
        self.packet_data_length_remaining = 0;
        self.packet_cmd = 0;
        self.packet_data.clear();
        self.packet_crc = 0;
        self.packet_crc_v2.reset();
    }
}

impl Default for MspParser {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: MspPayload + PartialEq + Sized> MspPacket<P> {
    /// Number of bytes that this packet requires to be packed
    pub fn packet_size_bytes(&self) -> usize {
        6 + self.data.as_ref().map_or(0, |p| p.len())
    }

    /// Number of bytes that this packet requires to be packed
    pub fn packet_size_bytes_v2(&self) -> usize {
        9 + self.data.as_ref().map_or(0, |p| p.len())
    }

    /// Serialize to network bytes
    pub fn serialize(&self, output: &mut [u8]) -> Result<(), MspError> {
        let l = output.len();

        if l != self.packet_size_bytes() {
            return Err(MspError::OutputBufferSizeMismatch);
        }

        output[0] = b'$';
        output[1] = b'M';
        output[2] = self.direction.into();
        output[3] = self.data.as_ref().as_ref().map_or(0, |p| p.len()) as u8;
        output[4] = self.cmd as u8;

        let data = self.data.as_ref().map_or(Ok(Vec::new()), |p| p.encode())?;
        output[5..l - 1].copy_from_slice(&data);

        let crc = data
            .iter()
            .fold(output[3] ^ output[4], |crc, byte| crc ^ byte);
        output[l - 1] = crc;

        Ok(())
    }

    /// Serialize to network bytes
    pub fn serialize_v2(&self, output: &mut [u8]) -> Result<(), MspError> {
        let l = output.len();

        if l != self.packet_size_bytes_v2() {
            return Err(MspError::OutputBufferSizeMismatch);
        }

        output[0] = b'$';
        output[1] = b'X';
        output[2] = self.direction.into();
        output[3] = 0;
        output[4..6].copy_from_slice(&self.cmd.to_le_bytes());
        output[6..8]
            .copy_from_slice(&(self.data.as_ref().map_or(0, |p| p.len()) as u16).to_le_bytes());

        let data = self.data.as_ref().map_or(Ok(Vec::new()), |p| p.encode())?;
        output[8..l - 1].copy_from_slice(&data);

        let mut crc = CRCu8::crc8dvb_s2();
        crc.digest(&output[3..l - 1]);
        output[l - 1] = crc.get_crc();

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::structs::*;

    #[test]
    fn test_serialize() {
        let packet = MspPacket::<MspVoltageMeter> {
            cmd: 2,
            direction: MspPacketDirection::Request,
            data: Some(MspVoltageMeter {
                id: 0xbe,
                value: 0xef,
            }),
        };

        let size = packet.packet_size_bytes();
        assert_eq!(8, size);

        let mut output = vec![0; size];
        packet.serialize(&mut output).unwrap();
        let expected = ['$' as u8, 'M' as u8, '<' as u8, 2, 2, 0xbe, 0xef, 81];
        assert_eq!(&expected, output.as_slice());

        let crc = 2 ^ 2 ^ 0xBE ^ 0xEF;
        assert_eq!(81, crc);

        let mut packet_parsed = None;
        let mut parser = MspParser::new();
        for b in output {
            let s = parser.parse(b);
            if let Ok(Some(p)) = s {
                packet_parsed = Some(p);
            }
        }

        assert_eq!(packet, packet_parsed.unwrap());
    }

    fn roundtrip<P: MspPayload + Sized + Debug>(packet: &MspPacket<P>) {
        let size = packet.packet_size_bytes();
        let mut output = vec![0; size];

        packet.serialize(&mut output).unwrap();

        let mut parser = MspParser::new();
        let mut packet_parsed = None;
        for b in output {
            let s = parser.parse(b);
            if let Ok(Some(p)) = s {
                packet_parsed = Some(p);
            }
        }
        assert_eq!(packet, &packet_parsed.unwrap());
    }

    #[test]
    fn test_roundtrip_empty_payload() {
        let packet = MspPacket::<MspStatus> {
            cmd: 200,
            direction: MspPacketDirection::Response,
            data: None,
        };
        roundtrip(&packet);
    }

    #[test]
    fn test_roundtrip_with_payload() {
        let payload = MspUniqueId {
            uid: *b"Holzcopter  ",
        };

        let packet = MspPacket {
            cmd: 1,
            direction: MspPacketDirection::Request,
            data: Some(payload),
        };
        roundtrip(&packet);
    }

    #[test]
    fn pure_bytes_to_msp_v2() {
        let buf = [0x24u8, 0x58, 0x3c, 0, 0x64, 0, 0, 0, 0x8f];

        let message: MspPacket<MspIdent> = MspPacket {
            cmd: 100,
            direction: MspPacketDirection::Request,
            data: None,
        };

        let mut parser = MspParser::new();
        let mut result = Ok(None);
        for byte in buf.iter() {
            result = parser.parse::<MspIdent>(*byte);
        }

        let new_message = result
            .expect("unable to decode new_message")
            .expect("did not receive a message");
        let mut new_buf = [0u8; 9];

        message
            .serialize_v2(&mut new_buf[..])
            .expect("unable to encode message");

        assert_eq!(buf, new_buf);
        assert_eq!(message, new_message);
    }
}
