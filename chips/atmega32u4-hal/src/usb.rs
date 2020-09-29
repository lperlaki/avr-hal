use crate::avr_hal::avr_device::interrupt::Mutex;

pub struct UsbBus {
    peripheral: Mutex<crate::atmega32u4::USB_DEVICE>,
}

impl UsbBus {
    pub fn new(peripheral: crate::atmega32u4::USB_DEVICE) -> Self {
        Self {
            peripheral: Mutex::new(peripheral),
        }
    }

    // Needs bare-metal = 1.0
    // pub fn release(self) -> crate::atmega32u4::USB_DEVICE {
    //     self.peripheral.into
    // }
}
use usb_device::{
    bus::PollResult,
    endpoint::{EndpointAddress, EndpointType},
    Result, UsbDirection,
};
impl usb_device::bus::UsbBus for UsbBus {
    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> Result<EndpointAddress> {
        todo!()
    }

    fn enable(&mut self) {
        todo!()
    }

    fn reset(&self) {
        todo!()
    }

    fn set_device_address(&self, addr: u8) {
        todo!()
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize> {
        todo!()
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {
        todo!()
    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        todo!()
    }

    fn suspend(&self) {
        todo!()
    }

    fn resume(&self) {
        todo!()
    }

    fn poll(&self) -> PollResult {
        todo!()
    }
}
