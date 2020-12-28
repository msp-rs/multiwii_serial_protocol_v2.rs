use crc_any::CRCu8;
use crate::prelude::v1::*;

/// Packet parsing error
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MspPacketParseError {
    OutputBufferSizeMismatch,
    CrcMismatch { expected: u8, calculated: u8 },
    InvalidData,
    InvalidHeader1,
    InvalidHeader2,
    InvalidDirection,
    InvalidDataLength,
}

/// Packet's desired destination
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MspPacketDirection {
    /// Network byte '<'
    ToFlightController,
    /// Network byte '>'
    FromFlightController,
    /// Network byte '!'
    Unsupported,
}

impl MspPacketDirection {
    /// To network byte
    pub fn to_byte(&self) -> u8 {
        let b = match *self {
            MspPacketDirection::ToFlightController => '<',
            MspPacketDirection::FromFlightController => '>',
            MspPacketDirection::Unsupported => '!',
        };
        b as u8
    }
}

#[derive(Debug, Clone, PartialEq)]
/// A decoded MSP packet, with a command code, direction and payload
pub struct MspPacket {
    pub cmd: u16,
    pub direction: MspPacketDirection,
    pub data: Vec<u8>,
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

#[derive(Copy, Clone, PartialEq, Debug)]
enum MSPVersion {
    V1,
    V2,
}

#[derive(Debug)]
/// Parser that can find packets from a raw byte stream
pub struct MspParser {
    state: MspParserState,
    packet_version: MSPVersion,
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
            packet_version: MSPVersion::V1,
            packet_direction: MspPacketDirection::ToFlightController,
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
    pub fn parse(&mut self, input: u8) -> Result<Option<MspPacket>, MspPacketParseError> {
        match self.state {
            MspParserState::Header1 => {
                if input == b'$' {
                    self.state = MspParserState::Header2;
                } else {
                    self.reset();
                }
            }

            MspParserState::Header2 => {
                self.packet_version = match input as char {
                    'M' => MSPVersion::V1,
                    'X' => MSPVersion::V2,
                    _ => {
                        self.reset();
                        return Err(MspPacketParseError::InvalidHeader2);
                    }
                };

                self.state = MspParserState::Direction;
            }

            MspParserState::Direction => {
                match input {
                    60 => self.packet_direction = MspPacketDirection::ToFlightController, // '>'
                    62 => self.packet_direction = MspPacketDirection::FromFlightController, // '<'
                    33 => self.packet_direction = MspPacketDirection::Unsupported, // '!' error
                    _ => {
                        self.reset();
                        return Err(MspPacketParseError::InvalidDirection);
                    }
                }

                self.state = match self.packet_version {
                    MSPVersion::V1 => MspParserState::DataLength,
                    MSPVersion::V2 => MspParserState::FlagV2,
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
                if self.packet_version == MSPVersion::V2 {
                    self.packet_crc_v2.digest(&self.packet_data);
                    self.packet_crc = self.packet_crc_v2.get_crc();
                }

                let packet_crc = self.packet_crc;
                if input != packet_crc {
                    self.reset();
                    return Err(MspPacketParseError::CrcMismatch {
                        expected: input,
                        calculated: packet_crc,
                    });
                }

                let mut n = Vec::new();
                mem::swap(&mut self.packet_data, &mut n);

                let packet = MspPacket {
                    cmd: self.packet_cmd,
                    direction: self.packet_direction,
                    data: n,
                };

                self.reset();

                return Ok(Some(packet));
            }
        }

        Ok(None)
    }

    pub fn reset(&mut self) {
        self.state = MspParserState::Header1;
        self.packet_direction = MspPacketDirection::ToFlightController;
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

impl MspPacket {
    /// Number of bytes that this packet requires to be packed
    pub fn packet_size_bytes(&self) -> usize {
        6 + self.data.len()
    }

    /// Number of bytes that this packet requires to be packed
    pub fn packet_size_bytes_v2(&self) -> usize {
        9 + self.data.len()
    }

    /// Serialize to network bytes
    pub fn serialize(&self, output: &mut [u8]) -> Result<(), MspPacketParseError> {
        let l = output.len();

        if l != self.packet_size_bytes() {
            return Err(MspPacketParseError::OutputBufferSizeMismatch);
        }

        output[0] = b'$';
        output[1] = b'M';
        output[2] = self.direction.to_byte();
        output[3] = self.data.len() as u8;
        output[4] = self.cmd as u8;

        output[5..l - 1].copy_from_slice(&self.data);

        let mut crc = output[3] ^ output[4];
        for b in &*self.data {
            crc ^= *b;
        }
        output[l - 1] = crc;

        Ok(())
    }

    /// Serialize to network bytes
    pub fn serialize_v2(&self, output: &mut [u8]) -> Result<(), MspPacketParseError> {
        let l = output.len();

        if l != self.packet_size_bytes_v2() {
            return Err(MspPacketParseError::OutputBufferSizeMismatch);
        }

        output[0] = b'$';
        output[1] = b'X';
        output[2] = self.direction.to_byte();
        output[3] = 0;
        output[4..6].copy_from_slice(&self.cmd.to_le_bytes());
        output[6..8].copy_from_slice(&(self.data.len() as u16).to_le_bytes());

        output[8..l - 1].copy_from_slice(&self.data);

        let mut crc = CRCu8::crc8dvb_s2();
        crc.digest(&output[3..l - 1]);
        output[l - 1] = crc.get_crc();

        Ok(())
    }
}

#[test]
fn test_serialize() {
    let packet = MspPacket {
        cmd: 2,
        direction: MspPacketDirection::ToFlightController,
        data: vec![0xbe, 0xef],
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

#[test]
fn test_roundtrip() {
    fn roundtrip(packet: &MspPacket) {
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

    {
        let packet = MspPacket {
            cmd: 1,
            direction: MspPacketDirection::ToFlightController,
            data: vec![0x00, 0x00, 0x00],
        };
        roundtrip(&packet);
    }

    {
        let packet = MspPacket {
            cmd: 200,
            direction: MspPacketDirection::FromFlightController,
            data: vec![],
        };
        roundtrip(&packet);
    }

    {
        let packet = MspPacket {
            cmd: 100,
            direction: MspPacketDirection::Unsupported,
            data: vec![0x44, 0x20, 0x00, 0x80],
        };
        roundtrip(&packet);
    }
}
