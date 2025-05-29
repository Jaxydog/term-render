// SPDX-License-Identifier: GPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later
// version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
// warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with this program. If not, see
// <https://www.gnu.org/licenses/>.

#![feature(array_chunks)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, StdoutLock, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::cursor::{MoveToColumn, MoveToRow};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType};
use directories::ProjectDirs;
use fontconfig::Fontconfig;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, LumaA, Pixel, Rgba};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};

const CHARACTER_RANGE: (char, char) = ('\u{20}', '\u{7F}');
const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(1_000 / 60);

static DIRECTORIES: LazyLock<ProjectDirs> = LazyLock::new(|| {
    ProjectDirs::from("dev.jaxydog", "", env!("CARGO_BIN_NAME")).expect("failed to resolve home directory")
});
static FONT_CONFIG: LazyLock<Fontconfig> = LazyLock::new(|| Fontconfig::new().expect("failed to load fonts"));
static SCALE_CONTEXT: LazyLock<Mutex<ScaleContext>> = LazyLock::new(|| Mutex::new(ScaleContext::new()));

#[derive(Debug, Parser)]
struct Arguments {
    /// The path to an image.
    path: Box<Path>,

    /// Specifies the font used by the terminal during rendering for more accurate character brightnesses.
    #[arg(short, long)]
    font: Option<Box<str>>,

    /// Whether to clean up all caches before running.
    #[arg(short, long)]
    clean: bool,
    /// Whether to draw the image without color.
    #[arg(short, long)]
    plain: bool,
}

fn main() -> Result<()> {
    let arguments = Arguments::parse();

    if arguments.clean && std::fs::exists(DIRECTORIES.cache_dir())? {
        std::fs::remove_dir_all(DIRECTORIES.cache_dir())?;
    }

    let source_image = image::open(&arguments.path)?;
    let brightnesses = self::compute_brightnesses(arguments.font.as_deref().unwrap_or(""))?;

    crossterm::terminal::enable_raw_mode()?;

    let mut stdout = std::io::stdout().lock();

    self::draw_ascii_image(&mut stdout, &brightnesses, &source_image, crossterm::terminal::size()?, !arguments.plain)?;

    loop {
        match crossterm::event::poll(EVENT_POLL_TIMEOUT)?.then(crossterm::event::read).transpose()? {
            Some(Event::Key(
                KeyEvent { code: KeyCode::Char('q') | KeyCode::Esc, .. }
                | KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. },
            )) => break,
            Some(Event::Resize(w, h)) => {
                self::draw_ascii_image(&mut stdout, &brightnesses, &source_image, (w, h), !arguments.plain)?
            }
            _ => {}
        }
    }

    crossterm::terminal::disable_raw_mode()?;

    crossterm::execute!(stdout, ResetColor, Print('\n')).map_err(Into::into)
}

fn draw_ascii_image(
    stdout: &mut StdoutLock<'_>,
    brightnesses: &HashMap<char, u16>,
    source_image: &DynamicImage,
    terminal_size: (u16, u16),
    use_color: bool,
) -> Result<()> {
    let scaled_image = source_image
        .resize_exact(source_image.width() * 2, source_image.height(), FilterType::Triangle)
        .resize(terminal_size.0 as u32, terminal_size.1 as u32, FilterType::Triangle);

    crossterm::queue!(stdout, Clear(ClearType::All))?;

    for pixel_y in 0 .. scaled_image.height() {
        crossterm::queue!(stdout, MoveToRow(pixel_y as u16))?;

        for (pixel_x, pixel) in (0 .. scaled_image.width())
            .map(|pixel_x| (pixel_x, scaled_image.get_pixel(pixel_x, pixel_y)))
            .filter(|(_, pixel)| pixel.0[3] > 0)
        {
            let LumaA([luma, alpha]) = pixel.to_luma_alpha();
            let brightness = luma as u16 * alpha as u16;
            let character = brightnesses
                .iter()
                .map(|(c, b)| (c, b.abs_diff(brightness)))
                .min_by_key(|(_, b)| *b)
                .map(|(c, _)| *c)
                .unwrap_or(' ');

            if use_color {
                let color = Color::Rgb { r: pixel.0[0], g: pixel.0[1], b: pixel.0[2] };

                crossterm::queue!(stdout, SetForegroundColor(color))?;
            }

            crossterm::queue!(stdout, MoveToColumn(pixel_x as u16), Print(character))?;
        }
    }

    stdout.flush().map_err(Into::into)
}

fn compute_brightnesses(font_family: &str) -> Result<HashMap<char, u16>> {
    const MAX_BRIGHTNESS: u16 = u8::MAX as u16 * u8::MAX as u16;

    let font = FONT_CONFIG.find(font_family, None).unwrap_or_else(|| FONT_CONFIG.find("", None).expect("missing font"));
    let cache_path = DIRECTORIES.cache_dir().join("ascii").join(&font.name).with_extension("json");

    if let Ok(cache_file) = File::open(&cache_path).map(BufReader::new)
        && let Ok(cache_data) = serde_json::from_reader(cache_file)
    {
        return Ok(cache_data);
    } else if cache_path.try_exists()? {
        std::fs::remove_file(&cache_path)?;
    }

    let font_data = std::fs::read(&font.path)?;
    let font_ref = FontRef::from_index(&font_data, 0).expect("invalid font file");

    let mut render = Render::new(&[Source::ColorOutline(0), Source::ColorBitmap(StrikeWith::BestFit), Source::Outline]);

    render.default_color([0xFF; 4]);

    let bitmaps: HashMap<char, (u32, u32, Box<[u8]>)> = (CHARACTER_RANGE.0 ..= CHARACTER_RANGE.1)
        .into_par_iter()
        .filter(|character| !character.is_whitespace() && !character.is_control())
        .filter_map(|character| {
            let mut context = SCALE_CONTEXT.lock().unwrap();
            let mut glyph_scaler = context.builder(font_ref).build();

            let image = render.render(&mut glyph_scaler, font_ref.charmap().map(character))?;

            drop(context);

            Some((character, (image.placement.width, image.placement.height, image.data.into_boxed_slice())))
        })
        .collect();

    let maximum_width = bitmaps.values().map(|(width, ..)| *width).max().unwrap_or(0);
    let maximum_height = bitmaps.values().map(|(_, height, _)| *height).max().unwrap_or(0);
    let pixels_per_cell = maximum_width as u64 * maximum_height as u64;

    if pixels_per_cell == 0 {
        return Ok(HashMap::new());
    }

    let brightnesses_iterator = bitmaps.par_iter().map(|(character, (.., bitmap))| {
        let brightness = bitmap
            .array_chunks::<4>()
            .par_bridge()
            .copied()
            .map(|pixel| Rgba(pixel).to_luma_alpha())
            .fold_with(0, |brightness, LumaA([luma, alpha])| brightness + (luma as u64 * alpha as u64))
            .sum::<u64>()
            / pixels_per_cell;

        (*character, brightness as u16)
    });

    let mut brightnesses: HashMap<char, u16> = brightnesses_iterator.collect();
    let brightness_scale = brightnesses.values().max().copied().unwrap_or(0) as f64 / MAX_BRIGHTNESS as f64;

    brightnesses.values_mut().for_each(|value| *value = ((*value) as f64 / brightness_scale) as u16);

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cache_file = BufWriter::new(File::create(&cache_path)?);

    serde_json::to_writer(&mut cache_file, &brightnesses)?;

    Ok(brightnesses)
}
