#[allow(unused)]
#[allow(dead_code)]
use core::convert::TryInto;
use usb_device::class_prelude::*;
use usb_device::Result as UsbResult;

/// This should be used as `device_class` when building the `UsbDevice`.
pub const USB_CLASS_AUDIO: u8 = 0x01;
pub const USB_SUBCLASS_MIDISTREAMING: u8 = 0x03;
pub const MIDI_PROTOCOL_NONE: u8 = 0x00;

// MS Class-Specific Interface Descriptor Types
const CS_UNDEFINED: u8 = 0x20;
const CS_DEVICE: u8 = 0x21;
const CS_CONFIGURATION: u8 = 0x22;
const CS_STRING: u8 = 0x23;
const CS_ENDPOINT: u8 = 0x25;
const CS_GR_TRM_BLOCK: u8 = 0x26;

//MS Class-Specific Interface Descriptor Subtypes
const MS_DESCRIPTOR_UNDEFINED: u8 = 0x00;
const MS_HEADER: u8 = 0x01;
const MIDI_IN_JACK: u8 = 0x02;
const MIDI_OUT_JACK: u8 = 0x03;
const ELEMENT: u8 = 0x04;
//
const CS_INTERFACE: u8 = 0x24;
//
const REQ_SEND_ENCAPSULATED_COMMAND: u8 = 0x00;
const REQ_GET_ENCAPSULATED_COMMAND: u8 = 0x01;
const REQ_SET_LINE_CODING: u8 = 0x20;
const REQ_GET_LINE_CODING: u8 = 0x21;
const REQ_SET_CONTROL_LINE_STATE: u8 = 0x22;

/// Packet level implementation of a CDC-ACM serial port.
///
/// This class can be used directly and it has the least overhead due to directly reading and
/// writing USB packets with no intermediate buffers, but it will not act like a stream-like serial
/// port. The following constraints must be followed if you use this class directly:
///
/// - `read_packet` must be called with a buffer large enough to hold max_packet_size bytes, and the
///   method will return a `WouldBlock` error if there is no packet to be read.
/// - `write_packet` must not be called with a buffer larger than max_packet_size bytes, and the
///   method will return a `WouldBlock` error if the previous packet has not been sent yet.
/// - If you write a packet that is exactly max_packet_size bytes long, it won't be processed by the
///   host operating system until a subsequent shorter packet is sent. A zero-length packet (ZLP)
///   can be sent if there is no other data to send. This is because USB bulk transactions must be
///   terminated with a short packet, even if the bulk endpoint is used for stream-like data.
pub struct MidiClass<'a, B: UsbBus> {
    comm_if: InterfaceNumber,
    comm_ep: EndpointIn<'a, B>,
    data_if: InterfaceNumber,
    read_ep: EndpointOut<'a, B>,
    write_ep: EndpointIn<'a, B>,
}

impl<B: UsbBus> MidiClass<'_, B> {
    /// Creates a new CdcAcmClass with the provided UsbBus and max_packet_size in bytes. For
    /// full-speed devices, max_packet_size has to be one of 8, 16, 32 or 64.
    pub fn new(alloc: &UsbBusAllocator<B>, max_packet_size: u16) -> MidiClass<'_, B> {
        MidiClass {
            comm_if: alloc.interface(),
            comm_ep: alloc.interrupt(8, 255),
            data_if: alloc.interface(),
            read_ep: alloc.bulk(max_packet_size),
            write_ep: alloc.bulk(max_packet_size),
        }
    }

    /// Gets the maximum packet size in bytes.
    pub fn max_packet_size(&self) -> u16 {
        // The size is the same for both endpoints.
        self.read_ep.max_packet_size()
    }

    /// Writes a single packet into the IN endpoint.
    pub fn write_packet(&mut self, data: &[u8]) -> UsbResult<usize> {
        self.write_ep.write(data)
    }

    /// Reads a single packet from the OUT endpoint.
    pub fn read_packet(&mut self, data: &mut [u8]) -> UsbResult<usize> {
        self.read_ep.read(data)
    }

    /// Gets the address of the IN endpoint.
    pub(crate) fn write_ep_address(&self) -> EndpointAddress {
        self.write_ep.address()
    }
}

impl<B: UsbBus> UsbClass<B> for MidiClass<'_, B> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> UsbResult<()> {
        // writer.iad(
        //     self.comm_if,
        //     2,
        //     USB_CLASS_AUDIO,
        //     USB_SUBCLASS_MIDISTREAMING,
        //     MIDI_PROTOCOL_NONE,
        // )?;

        // B.3.1 Standard AC Interface Descriptor
        writer.interface(self.comm_if, USB_CLASS_AUDIO, 0x01, MIDI_PROTOCOL_NONE)?;
        // B.3.2 Class-specific AC Interface Descriptor
        writer.write(CS_INTERFACE, &[0x01, 0x00, 0x01, 0x09, 0x00, 0x01, 0x01])?;

        writer.interface(
            self.data_if,
            USB_CLASS_AUDIO,
            USB_SUBCLASS_MIDISTREAMING,
            MIDI_PROTOCOL_NONE,
        )?;
        writer.write(
            CS_INTERFACE,
            &[
                MIDI_IN_JACK, // bDescriptorSubtype
                0x01,         // bJackType
                0x01,         // bJackId
                0x00,         // iJack
            ],
        )?;
        writer.write(
            CS_INTERFACE,
            &[
                MIDI_IN_JACK, // bDescriptorSubtype
                0x02,         // bJackType
                0x02,         // bJackId
                0x00,         // iJack
            ],
        )?;

        writer.write(
            CS_INTERFACE,
            &[
                MIDI_OUT_JACK, // bDescriptorSubtype
                0x01,          // bJackType
                0x03,          // bJackId
                0x01,          // bNrInputPins
                0x02,          // BaSourceID
                0x01,          // BaSourcePin
                0x00,          // iJack (unused)
            ],
        )?;
        writer.write(
            CS_INTERFACE,
            &[
                MIDI_OUT_JACK, // bDescriptorSubtype
                0x02,          // bJackType
                0x04,          // bJackId
                0x01,          // bNrInputPins
                0x02,          // BaSourceID
                0x01,          // BaSourcePin
                0x00,          // iJack (unused)
            ],
        )?;
        writer.endpoint(&self.read_ep)?;
        writer.write(
            CS_ENDPOINT,
            &[
                0x01, // bDescriptorSubtype (MS_GENERAL)
                0x01, // bNumEmbMidiJack
                0x01, // bAssocJackId(1)
            ],
        )?;
        writer.endpoint(&self.write_ep)?;
        writer.write(
            CS_ENDPOINT,
            &[
                0x01, // bDescriptorSubtype (MS_GENERAL)
                0x01, // bNumEmbMidiJack
                0x03, // bAssocJackId(1)
            ],
        )?;

        Ok(())
    }

    fn control_in(&mut self, xfer: ControlIn<B>) {
        let req = xfer.request();

        if !(req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.comm_if) as u16)
        {
            return;
        }

        match req.request {
            // REQ_GET_ENCAPSULATED_COMMAND is not really supported - it will be rejected below.
            REQ_GET_LINE_CODING if req.length == 7 => {
                xfer.accept(|data| Ok(7)).ok();
            }
            _ => {
                xfer.reject().ok();
            }
        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();

        if !(req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.comm_if) as u16)
        {
            return;
        }

        match req.request {
            REQ_SEND_ENCAPSULATED_COMMAND => {
                // We don't actually support encapsulated commands but pretend we do for standards
                // compatibility.
                xfer.accept().ok();
            }
            REQ_SET_LINE_CODING if xfer.data().len() >= 7 => {
                xfer.accept().ok();
            }
            REQ_SET_CONTROL_LINE_STATE => {
                xfer.accept().ok();
            }
            _ => {
                xfer.reject().ok();
            }
        };
    }
}
