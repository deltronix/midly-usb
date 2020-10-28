//! Provides utilities to read and write "live" MIDI messages produced in real-time, in contrast
//! with "dead" MIDI messages as stored in a `.mid` file.
//!
//! [`LiveEvent`](enum.LiveEvent.html) is very similar to
//! [`TrackEventKind`](../enum.TrackEventKind.html), except for subtle differences such as system
//! realtime messages, which can only exist in live events, or escape sequences, which can only
//! exist within track events.
//!
//! Usually OS APIs (and notably [`midir`](https://docs.rs/midir)) produce MIDI messages as a slice
//! of raw MIDI bytes, which can be parsed through
//! [`LiveEvent::parse`](enum.LiveEvent.html#method.parse) and written through
//! [`LiveEvent::write`](enum.LiveEvent.html#method.write).
//!
//! Note that MIDI byte streams, which are not clearly delimited packets, must be parsed through
//! the [`stream`](../stream/index.html) api.

use crate::{event::MidiMessage, prelude::*};

/// A live event produced by an OS API or generated on-the-fly, in contrast with "dead"
/// [`TrackEvent`](../struct.TrackEvent.html)s stored in a `.mid` file.
///
/// See the [`live`](index.html) module for more information.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LiveEvent<'a> {
    /// A MIDI message associated with a channel, carrying musical data.
    Midi { channel: u4, message: MidiMessage },
    /// A System Common message, as defined by the MIDI spec, including System Exclusive events.
    Common(SystemCommon<'a>),
    /// A one-byte System Realtime message.
    Realtime(SystemRealtime),
}
impl<'a> LiveEvent<'a> {
    /// Parse a complete MIDI message from its raw bytes.
    ///
    /// This method can be used to parse raw MIDI bytes coming from an OS API.
    /// Also see the example in the root crate documentation.
    pub fn parse(mut raw: &'a [u8]) -> Result<LiveEvent<'a>> {
        let status = raw
            .split_checked(1)
            .ok_or_else(|| err_invalid!("no status byte"))?[0];
        let data = u7::slice_from_int(raw);
        Self::read(status, data)
    }

    pub(crate) fn read(status: u8, data: &[u7]) -> Result<LiveEvent> {
        match status {
            0x80..=0xEF => {
                let data = MidiMessage::get_data_u7(status, data)?;
                let (channel, message) = MidiMessage::read(status, data);
                Ok(LiveEvent::Midi { channel, message })
            }
            _ => {
                //System Common event
                let ev = SystemCommon::read(status, data)?;
                Ok(LiveEvent::Common(ev))
            }
        }
    }

    /// Write a standalone message to the given output.
    ///
    /// This method can be used to write messages to be consumed by OS APIs.
    /// Also see the example in the root crate documentation.
    pub fn write<W: Write>(&self, out: &mut W) -> IoResult<W> {
        self.write_with_running_status(&mut None, out)
    }

    /// Write a message, skipping the status if it shares the status with the previous message.
    ///
    /// Note that it's usually discouraged to feed messages with running status to OS APIs.
    pub fn write_with_running_status<W: Write>(
        &self,
        running_status: &mut Option<u8>,
        out: &mut W,
    ) -> IoResult<W> {
        match self {
            LiveEvent::Midi { channel, message } => {
                let status = message.status_nibble() << 4 | channel.as_int();
                if Some(status) != *running_status {
                    *running_status = Some(status);
                    out.write(&[status])?;
                }
                message.write(out)?;
            }
            LiveEvent::Common(common) => {
                *running_status = None;
                common.write(out)?;
            }
            LiveEvent::Realtime(realtime) => {
                out.write(&[realtime.encode()])?;
            }
        }
        Ok(())
    }

    /// Write a standalone message to the given `std::io::Write` output.
    pub fn write_std<W: io::Write>(&self, out: W) -> io::Result<()> {
        self.write(&mut IoWrap(out))
    }

    /// Write a message, skipping the status if it shares the status with the previous message.
    ///
    /// Note that it's usually discouraged to feed messages with running status to OS APIs.
    pub fn write_std_with_running_status<W: io::Write>(
        &self,
        running_status: &mut Option<u8>,
        out: W,
    ) -> io::Result<()> {
        self.write_with_running_status(running_status, &mut IoWrap(out))
    }
}

/// System Realtime messages are one-byte messages that only occur within live MIDI streams.
/// They are usually time-sensitive, get top priority and can even be transmitted in between other
/// messages.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SystemRealtime {
    /// If sent, they should be sent 24 times per quarter note.
    TimingClock,
    Start,
    Continue,
    Stop,
    /// Once one of these messages is transmitted, a message should arrive every 300ms or else the
    /// connection is considered broken.
    ActiveSensing,
    Reset,
    /// An unknown system realtime message, with the given id byte.
    Undefined(u8),
}
impl SystemRealtime {
    /// Create a system realtime event from its id byte.
    pub fn new(status: u8) -> SystemRealtime {
        use SystemRealtime::*;
        match status {
            0xF8 => TimingClock,
            0xFA => Start,
            0xFB => Continue,
            0xFC => Stop,
            0xFE => ActiveSensing,
            0xFF => Reset,
            _ => {
                //Unknown system realtime event
                Undefined(status)
            }
        }
    }

    /// Get the id byte for this system realtime message.
    pub fn encode(self) -> u8 {
        use SystemRealtime::*;
        match self {
            TimingClock => 0xF8,
            Start => 0xFA,
            Continue => 0xFB,
            Stop => 0xFC,
            ActiveSensing => 0xFE,
            Reset => 0xFF,
            Undefined(byte) => byte,
        }
    }
}

/// A "system common event", as defined by the MIDI spec.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SystemCommon<'a> {
    /// A system-exclusive event.
    ///
    /// System Exclusive events start with a `0xF0` byte and finish with a `0xF7` byte, but this
    /// slice does not include either: it only includes data bytes in the `0x00..=0x7F` range.
    SysEx(&'a [u7]),
    /// A MIDI Time Code Quarter Frame message, carrying a tag type and a 4-bit tag value.
    MidiTimeCodeQuarterFrame(MtcQuarterFrameMessage, u4),
    /// The number of MIDI beats (6 x MIDI clocks) that have elapsed since the start of the
    /// sequence.
    SongPosition(u14),
    /// Select a given song index.
    SongSelect(u7),
    TuneRequest,
    /// An undefined System Common message, with arbitrary data bytes.
    Undefined(u8, &'a [u7]),
}
impl<'a> SystemCommon<'a> {
    #[allow(clippy::len_zero)]
    fn read(status: u8, data: &'a [u7]) -> Result<SystemCommon<'a>> {
        let ev = match status {
            0xF0 => {
                //SysEx
                SystemCommon::SysEx(&data[..])
            }
            0xF1 if data.len() >= 1 => {
                //MTC Quarter Frame
                SystemCommon::MidiTimeCodeQuarterFrame(
                    MtcQuarterFrameMessage::from_code(data[0].as_int() >> 4).unwrap(),
                    u4::from(data[0].as_int()),
                )
            }
            0xF2 if data.len() >= 2 => {
                //Song Position
                SystemCommon::SongPosition(u14::from(
                    (data[0].as_int() as u16) | ((data[1].as_int() as u16) << 7),
                ))
            }
            0xF3 if data.len() >= 1 => {
                //Song Select
                SystemCommon::SongSelect(data[0])
            }
            0xF6 => {
                //Tune Request
                SystemCommon::TuneRequest
            }
            0xF1..=0xF5 => {
                //Unknown system common event
                SystemCommon::Undefined(status, &data[..])
            }
            _ => {
                //Invalid/Unknown/Unreachable event
                //(Including F7 SysEx End Marker)
                bail!(err_invalid!("invalid status byte"))
            }
        };
        Ok(ev)
    }

    fn write<W: Write>(&self, out: &mut W) -> IoResult<W> {
        match self {
            SystemCommon::SysEx(data) => {
                out.write(&[0xF0])?;
                out.write(u7::slice_as_int(data))?;
                out.write(&[0xF7])
            }
            SystemCommon::MidiTimeCodeQuarterFrame(msgtype, data) => {
                out.write(&[0xF1, msgtype.as_code() << 4 | data.as_int()])
            }
            SystemCommon::SongPosition(pos) => {
                out.write(&[0xF2, pos.as_int() as u8 & 0x7F, (pos.as_int() >> 7) as u8])
            }
            SystemCommon::SongSelect(song) => out.write(&[0xF3, song.as_int()]),
            SystemCommon::TuneRequest => out.write(&[0xF6]),
            SystemCommon::Undefined(status, data) => {
                out.write(&[*status])?;
                out.write(u7::slice_as_int(data))
            }
        }
    }
}

/// The different kinds of info a Midi Time Code Quarter Frame message can carry.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MtcQuarterFrameMessage {
    /// The low nibble of the frame count.
    FramesLow,
    /// The high nibble of the frame count.
    FramesHigh,
    /// The low nibble of the second count.
    SecondsLow,
    /// The high nibble of the second count.
    SecondsHigh,
    /// The low nibble of the minute count.
    MinutesLow,
    /// The high nibble of the minute count.
    MinutesHigh,
    /// The low nibble of the hour count.
    HoursLow,
    /// The high nibble of the hour count.
    HoursHigh,
}
impl MtcQuarterFrameMessage {
    fn as_code(self) -> u8 {
        use MtcQuarterFrameMessage::*;
        match self {
            FramesLow => 0,
            FramesHigh => 1,
            SecondsLow => 2,
            SecondsHigh => 3,
            MinutesLow => 4,
            MinutesHigh => 5,
            HoursLow => 6,
            HoursHigh => 7,
        }
    }
    fn from_code(code: u8) -> Option<MtcQuarterFrameMessage> {
        use MtcQuarterFrameMessage::*;
        Some(match code {
            0 => FramesLow,
            1 => FramesHigh,
            2 => SecondsLow,
            3 => SecondsHigh,
            4 => MinutesLow,
            5 => MinutesHigh,
            6 => HoursLow,
            7 => HoursHigh,
            _ => return None,
        })
    }
}
