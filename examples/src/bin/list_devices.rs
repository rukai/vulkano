extern crate vulkano;
extern crate vulkano_win;

use vulkano::instance::{Instance, PhysicalDevice};

fn main() {
    let instance = {
        let extensions = vulkano_win::required_extensions();
        Instance::new(None, &extensions, None).expect("failed to create Vulkan instance")
    };

    for dev in PhysicalDevice::enumerate(&instance) {
        println!("device: {} (type: {:?})", dev.name(), dev.ty());
    }
}
