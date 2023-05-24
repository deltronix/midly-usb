use crate::io::{Write, WriteResult};
use crate::live::LiveEvent;
use crate::num::{u4, u7};
use crate::usb::*;
use crate::MidiMessage;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct UsbMidiPacket<'a> {
    pub cable_number: u4,
    pub code_index_number: CIN,
    pub event: LiveEvent<'a>,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum CIN {
    MiscFunction = 0x0,
    CableEvent = 0x1,
    TwoByteSysCom = 0x2,
    ThreeByteSysCom = 0x3,
    SysExStartOrContinue = 0x4,
    SingleByteSysComOrSysExEnd = 0x5,
    TwoByteSysExEnd = 0x6,
    ThreeByteSysExEnd = 0x7,
    NoteOff = 0x8,
    NoteOn = 0x9,
    PolyKeypress = 0xA,
    ControlChange = 0xB,
    ProgramChange = 0xC,
    ChannelPressure = 0xD,
    PitchbendChange = 0xE,
    SingleByte = 0xF,
}

impl CIN {
    pub fn from(cin: u4) -> CIN {
        match cin.as_int() {
            0x0 => Self::MiscFunction,
            0x1 => Self::CableEvent,
            0x2 => Self::TwoByteSysCom,
            0x3 => Self::ThreeByteSysCom,
            0x4 => Self::SysExStartOrContinue,
            0x5 => Self::SingleByteSysComOrSysExEnd,
            0x6 => Self::TwoByteSysExEnd,
            0x7 => Self::ThreeByteSysExEnd,
            0x8 => Self::NoteOff,
            0x9 => Self::NoteOn,
            0xA => Self::PolyKeypress,
            0xB => Self::ControlChange,
            0xC => Self::ProgramChange,
            0xD => Self::ChannelPressure,
            0xE => Self::PitchbendChange,
            0xF => Self::SingleByte,
            _ => Self::MiscFunction, // Won't happen in a u4
        }
    }
    pub fn as_int(&self) -> u8 {
        match self {
            Self::MiscFunction => 0x0,
            Self::CableEvent => 0x1,
            Self::TwoByteSysCom => 0x2,
            Self::ThreeByteSysCom => 0x3,
            Self::SysExStartOrContinue => 0x4,
            Self::SingleByteSysComOrSysExEnd => 0x5,
            Self::TwoByteSysExEnd => 0x6,
            Self::ThreeByteSysExEnd => 0x7,
            Self::NoteOff => 0x8,
            Self::NoteOn => 0x9,
            Self::PolyKeypress => 0xA,
            Self::ControlChange => 0xB,
            Self::ProgramChange => 0xC,
            Self::ChannelPressure => 0xD,
            Self::PitchbendChange => 0xE,
            Self::SingleByte => 0xF,
        }
    }
}

impl<'a> UsbMidiPacket<'a> {
    pub fn new(cn: u4, cin: CIN, event: LiveEvent) -> UsbMidiPacket {
        let ube = UsbMidiPacket {
            cable_number: cn,
            code_index_number: cin,
            event,
        };
        return ube;
    }
    pub(crate) fn packet_length(cin: CIN) -> usize {
        match cin {
            CIN::MiscFunction | CIN::CableEvent => 0,
            CIN::TwoByteSysCom => 2,
            CIN::ThreeByteSysCom | CIN::SysExStartOrContinue => 3,
            CIN::SingleByteSysComOrSysExEnd => 1,
            CIN::TwoByteSysExEnd => 2,
            CIN::ThreeByteSysExEnd => 3,
            CIN::NoteOff
            | CIN::NoteOn
            | CIN::PolyKeypress
            | CIN::ControlChange
            | CIN::PitchbendChange => 3,
            CIN::ChannelPressure | CIN::ProgramChange => 2,
            CIN::SingleByte => 1,
        }
    }
    pub fn read(packet: &'a [u8]) -> UsbMidiPacket {
        let cn: u4 = u4::from_int_lossy(packet[0] >> 4);
        let cin = CIN::from(u4::from_int_lossy(packet[0]));
        let status: u8 = packet[1];
        let data: &[u8] = &packet[1..=Self::packet_length(cin)];

        UsbMidiPacket {
            cable_number: cn,
            code_index_number: cin,
            event: LiveEvent::parse(data).expect("invalid data"),
        }
    }
    pub fn write<W: Write>(&self, out: &mut W) -> WriteResult<W> {
        let packet_header: u8 = self.code_index_number.as_int() | (self.cable_number.as_int() << 4);
        out.write(&[packet_header])?;
        match self.event {
            LiveEvent::Midi { channel, message } => {
                let status = message.status_nibble() << 4 | channel.as_int();
                out.write(&[status])?;
                message.write(out)?;
            }
            LiveEvent::Common(common) => {
                common.write(out)?;
            }
            LiveEvent::Realtime(realtime) => {
                out.write(&[realtime.encode()])?;
            }
        }

        let pad_length = 3 - Self::packet_length(self.code_index_number);
        for i in 0..pad_length {
            out.write(&[0x00])?;
        }
        Ok(())
    }
}
