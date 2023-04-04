use crate::live::{LiveEvent, SystemCommon, SystemRealtime};
use crate::{event::MidiMessage, prelude::*};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct UsbMidiEvent<'a> {
    cable_number: u4,
    code_index_number: u4,
    event: LiveEvent<'a>,
}

impl<'a> UsbMidiEvent<'a> {
    pub fn new(cn: u4, cin: u4, event: LiveEvent) -> UsbMidiEvent {
        let ube = UsbMidiEvent {
            cable_number: cn,
            code_index_number: cin,
            event,
        };
        return ube;
    }
    pub(crate) fn read(packet_header: u8, midi: &'a [u8; 3]) -> Result<UsbMidiEvent> {
        let cn: u4 = u4::from_int_lossy(packet_header >> 4);
        let cin: u4 = u4::from_int_lossy(packet_header);
        let status: u8 = midi[0];
        let data: &[u7] = u7::slice_from_int(&midi[1..2]);
        let event: Option<LiveEvent> = match cin.as_int() {
            0x0..=0x1 => None,
            0x2..=0x3 => {
                let ev = SystemCommon::read(status, data)?;
                Some(LiveEvent::Common(ev))
            }
            0x4..=0x7 => None,
            0x8..=0xE => {
                let data = MidiMessage::get_data_u7(status, data)?;
                let (channel, message) = MidiMessage::read(status, data);
                Some(LiveEvent::Midi { channel, message })
            }
            0xF => {
                let ev = SystemRealtime::new(status);
                Some(LiveEvent::Realtime(ev))
            }
            _ => None,
        };
        Ok(UsbMidiEvent {
            cable_number: cn,
            code_index_number: cin,
            event: event.expect("invalid cin in usb packet"),
        })
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
        // Add padding bytes
        match self.code_index_number.as_int() {
            0x0 | 0x1 => out.write(&[0x00; 3]),
            0x2 | 0x6 => out.write(&[0x00]),
            0xC..=0xD => out.write(&[0x00]),
            0xF | 0x5 => out.write(&[0x00; 2]),
            _ => Ok(()),
        }
    }
}
