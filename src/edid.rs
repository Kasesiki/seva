use std::io;

fn ensure(pred: bool, msg: &'static str) -> std::io::Result<()> {
    if pred {
        Ok(())
    } else {
        Err(std::io::Error::new(io::ErrorKind::InvalidData, msg))
    }
}

#[derive(Debug)]
pub struct EDID {
    pub manufacture_pnp: String,
    pub manufacture_week: Option<u8>,
    pub manufacture_year: u32,
    pub edid_version: (u8, u8),
    pub video_input: Option<VideoInput>,
    pub screen_size: Option<u8>,
    pub aspect_ratio: Option<String>,
    pub display_descriptors: Vec<DisplayTag>,
    pub resolution: Option<(u32, u32)>,
}

#[derive(Debug)]
pub enum DisplayTag {
    ProductSerialNumber(String),
    // (min vertical rate, max verical rate)
    DisplayRangeLimit((u16, u16)),
    ProductName(String),
}

pub fn display_tag_parse(r: &[u8]) -> Option<DisplayTag> {
    if r[0] + r[1] + r[2] != 0 {
        return None;
    }
    let tagname = match r[3] {
        0xFF => {
            let mut number = String::new();
            for i in &r[5..17] {
                if *i != 0x0A {
                    number.push(*i as char);
                } else {
                    break;
                }
            }
            DisplayTag::ProductSerialNumber(number)
        }
        0xFD => {
            let (min, max) = if r[4] & 0b11 == 0b11 {
                (r[5] as u16 + 255, r[6] as u16 + 255)
            } else {
                if r[4] & 0b11 == 0b10 {
                    (r[5] as u16, r[6] as u16 + 255)
                } else {
                    (r[5] as u16, r[6] as u16)
                }
            };
            DisplayTag::DisplayRangeLimit((min, max))
        }
        0xFC => {
            let mut name = String::new();
            for i in &r[5..17] {
                if *i != 0x0A {
                    name.push(*i as char);
                } else {
                    break;
                }
            }
            DisplayTag::ProductName(name)
        }
        _ => return None,
    };

    Some(tagname)
}

#[derive(Debug)]
pub struct VideoInput {
    pub color_depth: Option<u8>,
    pub interface: Option<String>,
}

impl EDID {
    pub fn parse(edidvec: Vec<u8>) -> io::Result<EDID> {
        ensure(edidvec.len() >= 128, "edid len < 128")?;
        ensure(
            edidvec[0..8] == [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00],
            "Invalid header.",
        )?;

        let mut manufacture_pnp = String::new();
        manufacture_pnp.push((b'A' + (edidvec[8] >> 2 & 0b00011111) - 1) as char);
        manufacture_pnp.push(
            (b'A' + (((edidvec[8] & 0b00000011) << 3) | (edidvec[9] >> 5 & 0b00000111)) - 1)
                as char,
        );
        manufacture_pnp.push((b'A' + (edidvec[9] & 0b00011111) - 1) as char);

        // todo id product code
        // todo id serial number

        let manufacture_week = if edidvec[16] >= 0x01 && edidvec[16] <= 0x36 {
            Some(edidvec[16])
        } else {
            None
        };

        // todo Video Input Definition (Analog), just Digital
        let video_input = if edidvec[20] >> 7 != 1 {
            None
        } else {
            let color_depth = match edidvec[20] >> 4 & 0b00000111 {
                0b001 => 6,
                0b010 => 8,
                0b011 => 10,
                0b100 => 12,
                0b101 => 14,
                0b110 => 16,
                _ => 0,
            };
            let interface = match edidvec[20] & 0b00001111 {
                0b0001 => "DVI",
                0b0010 => "HDMI-A",
                0b0011 => "HDMI-B",
                0b0100 => "MDDI",
                0b0101 => "DisplayPort",
                _ => "",
            };
            Some(VideoInput {
                color_depth: if color_depth == 0 {
                    None
                } else {
                    Some(color_depth)
                },
                interface: if interface.is_empty() {
                    None
                } else {
                    Some(String::from(interface))
                },
            })
        };

        let screen_size = if edidvec[0x15] == 0 || edidvec[0x16] == 0 {
            None
        } else {
            Some(
                ((((edidvec[0x15] as u32).pow(2) + (edidvec[0x16] as u32).pow(2)) as f32).sqrt()
                    / 2.54)
                    .round() as u8,
            )
        };

        //todo gamma

        //todo feature support

        //todo Display x, y Chromaticity Coordinates

        //todo Established Timings

        let (aspect_ratio, verts) = match edidvec[0x27] >> 6 {
            0b00 => ("16:10", vec![800, 900, 1050, 1200, 1600, 1800]),
            0b01 => ("4:3", vec![480, 600, 768, 960, 1050, 1200, 1536]),
            0b10 => ("5:4", vec![1024]),
            0b11 => ("16:9", vec![1080, 1400, 2160]),
            _ => ("", vec![]),
        };

        let mut descriptors = vec![];
        let mut resolution = None;
        

        if let Some(x) = display_tag_parse(&edidvec[0x48..=0x59]) {
            if let DisplayTag::DisplayRangeLimit((_, max)) = x {
                let mut vert = (edidvec[0x48+8] as u32) * 1000 / max as u32;
                for (id, i) in verts.iter().enumerate() {
                    if *i > vert && id != 0 {
                        vert = verts[id-1]
                    }
                }
                if !verts.contains(&vert) {
                    vert = verts[verts.len() - 1]
                }
                let hori = match edidvec[0x27] >> 6 {
                    0b00 => vert as f32 * 1.6,
                    0b01 => vert as f32 / 3.0 * 4.0,
                    0b10 => vert as f32 / 4.0 * 5.0,
                    0b11 => vert as f32 / 9.0 * 16.0,
                    _ => 0.0,
                };
                if hori != 0.0 {
                    resolution = Some((hori as u32, vert))
                }
            }
            descriptors.push(x);
        };
        if let Some(x) = display_tag_parse(&edidvec[0x5a..=0x6b]) {
            descriptors.push(x);
        };
        if let Some(x) = display_tag_parse(&edidvec[0x6c..=0x7d]) {
            descriptors.push(x);
        };

        Ok(EDID {
            manufacture_pnp,
            manufacture_week,
            manufacture_year: 1990 + (edidvec[17] as u32),
            edid_version: (edidvec[18], edidvec[19]),
            video_input,
            screen_size,
            aspect_ratio: if aspect_ratio.is_empty() {
                None
            } else {
                Some(String::from(aspect_ratio))
            },
            display_descriptors: descriptors,
            resolution
        })
    }
}

pub fn find_edid() -> std::io::Result<Vec<EDID>> {
    let dir = std::fs::read_dir("/sys/class/drm/")?;
    let mut result = vec![];
    for i in dir.flatten() {
        if !i.file_type()?.is_file() {
            for i in std::fs::read_dir(i.path())?.flatten() {
                if i.file_type()?.is_file()
                    && i.file_name() == "status"
                    && std::fs::read_to_string(i.path())?.trim() == "connected"
                {
                    let mut p = i.path();
                    p.pop();
                    p.push("edid");
                    let edidvec = std::fs::read(p)?;

                    let edid = EDID::parse(edidvec)?;

                    result.push(edid);
                }
            }
        }
    }
    Ok(result)
}
