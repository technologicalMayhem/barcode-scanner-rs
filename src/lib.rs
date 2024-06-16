//! Scan 1D barcodes using a hand scanner for Rust.
//!
//! The `barcode-scanner` crate provides an interface to USB barcode scanners on Linux.
//! It is built on top of the [`evdev`] crate.
//! It works with any barcode scanner that acts as a keyboard.
//!
//! Currently supported features:
//! * One [`BarcodeScanner`] struct for all USB hand scanners that operate as a keyboard.
//! * Prevent other clients from receiving events from the selected device by grabbing it.
//! * Read 1D barcode consisting of numbers and letters.
//! * Omit special characters in a barcode.
//!
//! # Example
//! This example grabs a hand scanner and prints a barcode that is read.
//!
//! ```no_run
//! # fn example() -> Result<(), barcode_scanner::Error> {
//!    use barcode_scanner::BarcodeScanner;
//!
//!    let mut scanner = BarcodeScanner::open("/dev/input/by-id/usb-USB_Adapter_USB_Device-event-kbd")?;
//!    loop {
//!        let barcode = scanner.read()?;
//!        println!("{}", barcode);
//!    }
//! # }
//! ```

use std::path::Path;

/// A barcode scanner.
pub struct BarcodeScanner {
	/// The underlying evdev device.
	device: evdev::Device,

	/// A buffer used to collect keystrokes in until a whole barcode has been read.
	buffer: String,
}

/// An error reported by the barcode scanner.
#[derive(Debug, Clone)]
pub struct Error {
	msg: String,
}

impl BarcodeScanner {
	/// Create a barcode scanner and grab the device by a device path
	///
	/// # Example
	/// ```no_run
	/// # use barcode_scanner::BarcodeScanner;
	/// # fn foo() -> Result<(), barcode_scanner::Error> {
	/// let mut scanner = BarcodeScanner::open("/dev/input/event18")?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
		let path = path.as_ref();
		let mut device = evdev::Device::open(path)
			.map_err(|e| Error::new(format!("Failed to open input device {}: {e}", path.display())))?;
		device.grab()
			.map_err(|e| Error::new(format!("Failed to grab input device {}: {e}", path.display())))?;

		Ok(Self {
			device,
			buffer: String::new(),
		})
	}

	/// Create a barcode scanner and grab the device by a physical device path
	///
	/// # Example
	/// ```no_run
	/// # use barcode_scanner::BarcodeScanner;
	/// # fn foo() -> Result<(), ()> {
	/// let device_path = "usb-0000:00:14.0-3/input0";
	/// let mut scanner = BarcodeScanner::open_by_physical_path(device_path)
	///     .map_err(|e| eprintln!("{}", e))?
	///     .ok_or_else(|| eprintln!("No such device: {device_path}"))?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn open_by_physical_path(physical_path: impl AsRef<str>) -> Result<Option<Self>, Error> {
		let physical_path = physical_path.as_ref();
		for (_path, mut device) in evdev::enumerate() {
			// Find the scanner among other USB devices by physical path.
			let device_physical_path = match device.physical_path() {
				Some(x) => x,
				None => continue,
			};
			if device_physical_path == physical_path {
				// Prevents other clients from receiving events from this device.
				device.grab()
					.map_err(|e| Error::new(format!("Failed to grab input device {physical_path}: {e}")))?;
				return Ok(Some(Self {
					device,
					buffer: String::new(),
				}))
			}
		}
		Ok(None)
	}

	/// Read a barcode from the device.
	///
	/// Blocks until an entire barcode has been read.
	///
	/// # Example
	/// ```no_run
	/// # use barcode_scanner::BarcodeScanner;
	/// # fn foo() -> Result<(), barcode_scanner::Error> {
	/// # let mut scanner = BarcodeScanner::open("/dev/input/event18")?;
	/// let barcode = scanner.read()?;
	/// println!("Barcode: {barcode}");
	/// # Ok(())
	/// # }
	pub fn read(&mut self) -> Result<String, Error> {
		loop {
			let events = self.device.fetch_events()
				.map_err(|e| Error::new(format!("Failed to fetch events from input device: {e}")))?;

			// Track the state of the shift keys and capslock
			let mut left_shift_pressed = false;
			let mut right_shift_pressed = false;
			let mut capslock_on = false;
			for event in events {
				// Check if key is pressed (value 1 for the key pressed, velue 0 for the key released).
				if event.event_type() == evdev::EventType::KEY {
					// Create Key object based on the code.
					let key_name = evdev::Key(event.code());

					match key_name {
						evdev::Key::KEY_LEFTSHIFT => left_shift_pressed = event.value() == 1,
						evdev::Key::KEY_RIGHTSHIFT => right_shift_pressed = event.value() == 1,
						evdev::Key::KEY_CAPSLOCK => capslock_on = event.value() == 1,
						_ => {},
					}					

                    // Map key_name to the number or char.
                    if event.value() == 1 {
                        if let Some(c) = key_to_str(key_name, left_shift_pressed || right_shift_pressed || capslock_on) {
                            self.buffer.push(c);
                        }
                    }
				}
			}

			if let Some(index) = self.buffer.find('\n') {
				let mut barcode: String= self.buffer.drain(..index + 1).collect();
				barcode.pop();
				return Ok(barcode);
			}
		}
	}

	/// Convert the device into a asynchonous stream of read barcodes.
	#[cfg(feature = "tokio")]
	pub fn into_async_stream(mut self) -> tokio::sync::mpsc::UnboundedReceiver<Result<String, Error>> {
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
		tokio::task::spawn_blocking(move || {
			loop {
				if tx.send(self.read()).is_err() {
					break;
				}
			}
		});
		rx
	}
}

/// Map a scanned key to a character
fn key_to_str(key: evdev::Key, capital: bool) -> Option<char> {
    let char = match key {
        // Digits
        evdev::Key::KEY_1 => ['1', '!'],
        evdev::Key::KEY_2 => ['2', '@'],
        evdev::Key::KEY_3 => ['3', '#'],
        evdev::Key::KEY_4 => ['4', '$'],
        evdev::Key::KEY_5 => ['5', '%'],
        evdev::Key::KEY_6 => ['6', '^'],
        evdev::Key::KEY_7 => ['7', '&'],
        evdev::Key::KEY_8 => ['8', '*'],
        evdev::Key::KEY_9 => ['9', '('],
        evdev::Key::KEY_0 => ['0', ')'],
        // Letters
        evdev::Key::KEY_A => ['a','A'],
        evdev::Key::KEY_B => ['b','B'],
        evdev::Key::KEY_C => ['c','C'],
        evdev::Key::KEY_D => ['d','D'],
        evdev::Key::KEY_E => ['e','E'],
        evdev::Key::KEY_F => ['f','F'],
        evdev::Key::KEY_G => ['g','G'],
        evdev::Key::KEY_H => ['h','H'],
        evdev::Key::KEY_I => ['i','I'],
        evdev::Key::KEY_J => ['j','J'],
        evdev::Key::KEY_K => ['k','K'],
        evdev::Key::KEY_L => ['l','L'],
        evdev::Key::KEY_M => ['m','M'],
        evdev::Key::KEY_N => ['n','N'],
        evdev::Key::KEY_O => ['o','O'],
        evdev::Key::KEY_P => ['p','P'],
        evdev::Key::KEY_Q => ['q','Q'],
        evdev::Key::KEY_R => ['r','R'],
        evdev::Key::KEY_S => ['s','S'],
        evdev::Key::KEY_T => ['t','T'],
        evdev::Key::KEY_U => ['u','U'],
        evdev::Key::KEY_V => ['v','V'],
        evdev::Key::KEY_W => ['w','W'],
        evdev::Key::KEY_X => ['x','X'],
        evdev::Key::KEY_Y => ['y','Y'],
        evdev::Key::KEY_Z => ['z','Z'],
        // Special
        evdev::Key::KEY_SPACE => [' ', ' '],
        evdev::Key::KEY_TAB => ['\t', '\t'],
        evdev::Key::KEY_APOSTROPHE => ['\'', '"'],
        evdev::Key::KEY_EQUAL => ['=', '+'],
        evdev::Key::KEY_COMMA => [',', '<'],
        evdev::Key::KEY_MINUS => ['-', '_'],
        evdev::Key::KEY_DOT => ['.', '>'],
        evdev::Key::KEY_SLASH => ['/', '?'],
        evdev::Key::KEY_BACKSLASH => ['\\', '|'],
        evdev::Key::KEY_SEMICOLON => [';', ':'],
        evdev::Key::KEY_LEFTBRACE => ['[', '{'],
        evdev::Key::KEY_RIGHTBRACE => [']', '}'],
        evdev::Key::KEY_GRAVE => ['`', '~'],
        evdev::Key::KEY_KPENTER => ['\n', '\n'],
        evdev::Key::KEY_ENTER => ['\n', '\n'],
        _ => return None
    };

    if capital {
        Some(char[1])
    } else {
        Some(char[0])
    }
}

impl Error {
	fn new(msg: String) -> Self {
		Self { msg }
	}
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.msg)
	}
}

impl std::error::Error for Error { }
