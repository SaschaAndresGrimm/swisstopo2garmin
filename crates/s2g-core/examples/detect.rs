//! Report what the app sees: mounted Garmin volumes, and Garmin devices on the USB bus.
//!
//!   cargo run -p s2g-core --example detect

fn main() {
    let mounted = s2g_core::devices::detect();
    println!("mounted as mass storage: {}", mounted.len());
    for d in &mounted {
        println!(
            "  {}  model={:?}  unit={:?}  free={:?}  maps={}",
            d.mount.display(),
            d.model,
            d.unit_id,
            d.free_bytes,
            d.existing_maps.len()
        );
    }

    let usb = s2g_core::devices::usb_devices();
    println!("\non the USB bus: {}", usb.len());
    for d in &usb {
        let mounted_too = mounted.iter().any(|m| {
            m.model
                .as_deref()
                .map(|x| x.eq_ignore_ascii_case(&d.model))
                .unwrap_or(false)
        });
        println!(
            "  {}  serial={:?}  {}",
            d.model,
            d.serial,
            if mounted_too {
                "(also mounted)"
            } else {
                "(NOT mounted -- MTP mode, or the volume has not appeared yet)"
            }
        );
    }
}
