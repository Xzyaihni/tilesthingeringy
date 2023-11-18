use std::{
    fs,
    mem,
    iter,
    thread,
    rc::Rc,
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
    ops::{Index, IndexMut}
};

use sdl2::{
    EventPump,
    event::Event,
    rect::Rect,
    video::{Window, WindowContext},
    render::{Canvas, Texture, TextureCreator, BlendMode},
    keyboard::Keycode,
    pixels::{
        PixelFormatEnum,
        Color as SdlColor
    }
};

use ui::{Ui, UiElement, UiElementType, ElementId, UiAnimatableId};
use container::Container2d;
use animator::{Animator, AnimatedValue, ValueAnimation};

pub use crate::image::Image;
pub use point::Point2;

mod point;
mod image;
mod container;
mod ui;

pub mod animator;


struct Camera
{
    pub pos: Point2<f32>,
    pub height: f32
}

impl Camera
{
    pub fn new(height: f32) -> Self
    {
        Self{pos: Point2::new(0.0, 0.0), height}
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tile(usize);

impl Tile
{
    pub fn new(id: usize) -> Self
    {
        Tile(id + 1)
    }

    pub fn none() -> Self
    {
        Self(0)
    }

    pub fn is_none(&self) -> bool
    {
        self.0 == 0
    }

    pub fn id(&self) -> usize
    {
        self.0
    }
}

struct Scene
{
    container: Container2d<Tile>,
    offset: Point2<i32>
}

impl Scene
{
    pub fn new(size: Point2<usize>, offset: Point2<i32>) -> Self
    {
        let container = Container2d::new(size);

        Self{container, offset}
    }

    pub fn extend_to_contain(&mut self, global_pos: Point2<i32>)
    {
        let pos = global_pos.map(|x| x as i32) + self.offset;

        let size = self.container.size().map(|x| x as i32);
        let distance = pos.zip(size).map(|(pos, size)|
        {
            if pos >= size
            {
                pos - size + 1
            } else if pos < 0
            {
                pos
            } else
            {
                0
            }
        });

        let new_size = size + distance.map(|x| x.abs());

        if new_size != size
        {
            let this_offset = distance.map(|x| if x < 0 { x } else { 0 });

            self.offset -= this_offset;

            let mut new_container = Container2d::new(new_size.map(|x| x as usize));

            for (pos, tile) in self.container.iter()
            {
                let new_pos = pos.map(|x| x as i32) - this_offset;

                new_container[new_pos.map(|x| x as usize)] = *tile;
            }

            self.container = new_container;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=(Point2<i32>, &Tile)>
    {
        self.container.iter().map(|(pos, tile)|
        {
            (pos.map(|x| x as i32) - self.offset, tile)
        })
    }

    fn to_local(&self, pos: Point2<i32>) -> Point2<usize>
    {
        let local = pos + self.offset;

        assert!(local.x >= 0 && local.y >= 0);

        local.map(|x| x as usize)
    }
}

impl Index<Point2<i32>> for Scene
{
    type Output = Tile;

    fn index(&self, index: Point2<i32>) -> &Self::Output
    {
        self.container.index(self.to_local(index))
    }
}

impl IndexMut<Point2<i32>> for Scene
{
    fn index_mut(&mut self, index: Point2<i32>) -> &mut Self::Output
    {
        self.extend_to_contain(index);

        self.container.index_mut(self.to_local(index))
    }
}

#[derive(Debug, Clone, Copy)]
enum ControlName
{
    Forward = 0,
    Back,
    Right,
    Left,
    ZoomOut,
    ZoomIn,
    CreateTile,
    DeleteTile,
    LAST
}

const FPS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureId(usize);

pub struct Assets
{
    creator: TextureCreator<WindowContext>,
    texture_ids: HashMap<PathBuf, usize>,
    tiles: Vec<TextureId>,
    // i despise the lifetime on the texture, this sdl wrapper is absolute CANCER
    textures: Vec<Texture<'static>>
}

impl Assets
{
    pub fn new(creator: TextureCreator<WindowContext>) -> Self
    {
        Self{
            creator,
            texture_ids: HashMap::new(),
            tiles: Vec::new(),
            textures: Vec::new()
        }
    }

    pub fn add_tile(&mut self, path: impl Into<PathBuf>)
    {
        let id = self.add_texture(path);

        self.tiles.push(id);
    }

    pub fn add_texture(&mut self, path: impl Into<PathBuf>) -> TextureId
    {
        let path = path.into();

        let id = self.textures.len();

        let image = Image::load(&path);

        let texture = unsafe{ self.texture_from_image(image) };
        self.textures.push(texture);

        self.texture_ids.insert(path, id);

        TextureId(id)
    }

    unsafe fn texture_from_image(&self, image: Image) -> Texture<'static>
    {
        let mut texture = self.creator.create_texture_static(
            PixelFormatEnum::RGBA32,
            image.size().x as u32,
            image.size().y as u32
        ).unwrap();
        texture.set_blend_mode(BlendMode::Blend);

        let data = image.data();

        texture.update(None, data, image.bytes_row()).unwrap();

        Self::make_texture_static(texture)
    }

    unsafe fn make_texture_static(texture: Texture<'_>) -> Texture<'static>
    {
        mem::transmute(texture)
    }

    pub fn texture_id(&self, name: impl AsRef<Path>) -> TextureId
    {
        TextureId(self.texture_ids[name.as_ref()])
    }

    pub fn tile_texture_id(&self, tile: Tile) -> TextureId
    {
        assert!(!tile.is_none());

        self.tiles[tile.id() - 1]
    }

    pub fn texture<'a>(&'a self, id: TextureId) -> &'a Texture<'static>
    {
        &self.textures[id.0]
    }
}

pub struct GameWindow
{
    window_size: Point2<u32>,
    canvas: Canvas<Window>,
    events: EventPump,
    assets: Rc<RefCell<Assets>>
}

impl GameWindow
{
    pub fn new(window_size: Point2<u32>) -> Self
    {
        let ctx = sdl2::init().unwrap();
        let video = ctx.video().unwrap();

        let window = video.window("tile thingeringy", window_size.x, window_size.y)
            .build()
            .unwrap();

        let canvas = window.into_canvas().build().unwrap();

        let events = ctx.event_pump().unwrap();

        let assets = Rc::new(RefCell::new(Assets::new(canvas.texture_creator())));

        Self{
            window_size,
            canvas,
            events,
            assets
        }
    }

    pub fn window_size(&self) -> &Point2<u32>
    {
        &self.window_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keybind
{
    Keyboard(Keycode),
    Mouse(u32)
}

impl From<Keycode> for Keybind
{
    fn from(value: Keycode) -> Self
    {
        Self::Keyboard(value)
    }
}

impl From<u32> for Keybind
{
    fn from(value: u32) -> Self
    {
        Self::Mouse(value)
    }
}

enum UiVariant
{
    Normal,
    Tiles
}

// giga super big struct cuz im lazy
struct Game
{
    window_size: Point2<usize>,
    camera: Camera,
    controls: [bool; ControlName::LAST as usize],
    scenes: Vec<Scene>,
    current_scene: usize,
    current_tile: Tile,
    window: Rc<RefCell<GameWindow>>,
    assets: Rc<RefCell<Assets>>,
    next_scene_button: ElementId,
    prev_scene_button: ElementId,
    current_tile_button: ElementId,
    tile_buttons: Vec<ElementId>,
    keybinds: Vec<(Keybind, ControlName)>,
    mouse_pos: Point2<i32>,
    ui: Ui,
    tiles_panel: ElementId,
    tiles_window_animator_open: Animator<UiAnimatableId>,
    tiles_window_animator_close: Animator<UiAnimatableId>,
    tiles_ui: Ui,
    current_ui: UiVariant
}

impl Game
{
    pub fn new(
        window_size: Point2<usize>,
        window: Rc<RefCell<GameWindow>>,
        tiles_amount: usize
    ) -> Self
    {
        let aspect = window_size.x as f32 / window_size.y as f32;

        let camera = Camera::new(10.0);

        let controls = [false; ControlName::LAST as usize];

        let scenes = Vec::new();

        let current_tile = Tile::new(0);

        let assets = window.borrow().assets.clone();

        let mut ui = Ui::new(window.clone(), assets.clone());

        let texture_id_inner = |name: String|
        {
            assets.borrow().texture_id(name)
        };

        let texture_id = |name: &str|
        {
            texture_id_inner(name.to_owned())
        };

        let tile_texture_id = |tile: Tile|
        {
            assets.borrow().tile_texture_id(tile)
        };

        let next_scene_button = ui.push(UiElement{
            kind: UiElementType::Button,
            pos: Point2::new(1.0 - 0.08, 1.0 - (0.07 * aspect)),
            size: Point2::new(0.08, 0.07 * aspect),
            texture: texture_id("ui/plus.png")
        });

        let prev_scene_button = ui.push(UiElement{
            kind: UiElementType::Button,
            pos: Point2::new(1.0 - (0.08 * 2.0) - 0.02, 1.0 - (0.07 * aspect)),
            size: Point2::new(0.08, 0.07 * aspect),
            texture: texture_id("ui/minus.png")
        });

        let current_tile_button;
        {
            let size = 0.1;
            let margin = size * 0.1;

            ui.push(UiElement{
                kind: UiElementType::Panel,
                pos: Point2::new(0.0, 1.0 - ((size + margin) * aspect)),
                size: Point2::new(size + margin, (size + margin) * aspect),
                texture: texture_id("ui/white.png")
            });

            ui.push(UiElement{
                kind: UiElementType::Panel,
                pos: Point2::new(0.0, 1.0 - (size * aspect)),
                size: Point2::new(size, size * aspect),
                texture: texture_id("ui/background.png")
            });

            current_tile_button = ui.push(UiElement{
                kind: UiElementType::Button,
                pos: Point2::new(0.0, 1.0 - (size * aspect)),
                size: Point2::new(size, size * aspect),
                texture: tile_texture_id(current_tile)
            });
        }

        let mut tiles_ui = Ui::new(window.clone(), assets.clone());

        let mut tile_buttons = Vec::with_capacity(tiles_amount);

        let margin = 0.1;
        let panel_size = 1.0 - margin * 2.0;

        let panel_size = if aspect < 1.0
        {
            panel_size
        } else
        {
            panel_size / aspect
        };

        let panel_size = Point2::new(panel_size, panel_size * aspect);
        let panel_pos = (-panel_size + 1.0) * 0.5;

        let tiles_panel;
        {
            tiles_panel = tiles_ui.push(UiElement{
                kind: UiElementType::Panel,
                pos: panel_pos,
                size: panel_size,
                texture: texture_id("ui/panel.png")
            });

            let items_row = (tiles_amount as f32).sqrt().ceil() as usize;

            for tile_id in 0..tiles_amount
            {
                let margin = 0.045;
                let padding = 0.1;

                let tile = Tile::new(tile_id);

                let item_pos = Point2::new(tile_id % items_row, tile_id / items_row);

                let row_size = items_row as f32 + (items_row - 1) as f32 * padding;
                let tile_size = (1.0 - margin * 2.0) / row_size;

                let padding = tile_size * padding;

                let mut tile_pos = item_pos.map(|x| x as f32) * (tile_size + padding);
                tile_pos.y = 1.0 - tile_pos.y - tile_size - margin;
                tile_pos.x += margin;

                let tile_element_id = tiles_ui.push_child(&tiles_panel, UiElement{
                    kind: UiElementType::Button,
                    pos: tile_pos,
                    size: Point2::repeat(tile_size),
                    texture: tile_texture_id(tile)
                });

                tile_buttons.push(tile_element_id);
            }
        }

        let tiles_window_animator_open;
        let tiles_window_animator_close;
        {
            let thin_line = panel_size.y * 0.02;

            let x_curve = ValueAnimation::EaseIn(0.7);
            let y_curve = ValueAnimation::EaseIn(0.9);

            let y_scale_start = 0.2;
            let x_scale_end = 0.4;

            tiles_window_animator_open = Animator::new(vec![
                AnimatedValue{
                    id: UiAnimatableId::ScaleY,
                    range: thin_line..=panel_size.y,
                    curve: y_curve.clone(),
                    duration: y_scale_start..=1.0
                },
                AnimatedValue{
                    id: UiAnimatableId::PositionY,
                    range: (panel_size.y / 2.0 + panel_pos.y)..=panel_pos.y,
                    curve: y_curve,
                    duration: y_scale_start..=1.0
                },
                AnimatedValue{
                    id: UiAnimatableId::ScaleX,
                    range: 0.0..=panel_size.x,
                    curve: x_curve.clone(),
                    duration: 0.0..=x_scale_end
                },
                AnimatedValue{
                    id: UiAnimatableId::PositionX,
                    range: (panel_size.x / 2.0 + panel_pos.x)..=panel_pos.x,
                    curve: x_curve,
                    duration: 0.0..=x_scale_end
                }
            ], Duration::from_millis(200));

            tiles_window_animator_close = tiles_window_animator_open.reversed();
        }

        let keybinds: Vec<(Keybind, _)> = vec![
            (Keycode::W.into(), ControlName::Forward),
            (Keycode::S.into(), ControlName::Back),
            (Keycode::A.into(), ControlName::Left),
            (Keycode::D.into(), ControlName::Right),
            (Keycode::Space.into(), ControlName::ZoomOut),
            (Keycode::LCtrl.into(), ControlName::ZoomIn),
            (0.into(), ControlName::CreateTile),
            (Keycode::Z.into(), ControlName::CreateTile),
            (2.into(), ControlName::DeleteTile),
            (Keycode::X.into(), ControlName::DeleteTile),
        ];

        let mut this = Self{
            window_size,
            camera,
            controls,
            scenes,
            current_scene: 0,
            current_tile,
            next_scene_button,
            prev_scene_button,
            current_tile_button,
            tile_buttons,
            keybinds,
            mouse_pos: Point2::new(0, 0),
            window,
            assets,
            ui,
            tiles_panel,
            tiles_window_animator_open,
            tiles_window_animator_close,
            tiles_ui,
            current_ui: UiVariant::Normal
        };

        this.ensure_current_tile();

        this
    }

    pub fn run(mut self)
    {
        loop
        {
            if !self.single_frame()
            {
                return;
            }

            thread::sleep(Duration::from_millis(1000 / FPS as u64));
        }
    }

    fn ensure_current_tile(&mut self)
    {
        let texture = self.assets.borrow().tile_texture_id(self.current_tile);

        *self.ui.get(&self.current_tile_button).borrow_mut().texture() = texture;
    }

    fn ensure_current_scene(&mut self)
    {
        while self.scenes.len() <= self.current_scene
        {
            let size = Point2::new(0, 0);
            let offset = Point2::new(0, 0);

            self.scenes.push(Scene::new(size, offset));
        }
    }

    fn single_frame(&mut self) -> bool
    {
        let window = self.window.clone();
        for event in window.borrow_mut().events.poll_iter()
        {
            if !self.on_event(event)
            {
                return false;
            }
        }

        self.ensure_current_scene();

        let dt = (1000 / FPS) as f32;
        let speed = 0.002 * self.camera.height.sqrt() * dt;

        if self.pressed(ControlName::Forward)
        {
            self.camera.pos.y += speed;
        } else if self.pressed(ControlName::Back)
        {
            self.camera.pos.y -= speed;
        }

        if self.pressed(ControlName::Right)
        {
            self.camera.pos.x += speed;
        } else if self.pressed(ControlName::Left)
        {
            self.camera.pos.x -= speed;
        }

        let zoom_scale = 0.9_f32.powf(0.05 * dt);

        if self.pressed(ControlName::ZoomOut)
        {
            self.camera.height /= zoom_scale;
        } else if self.pressed(ControlName::ZoomIn)
        {
            self.camera.height *= zoom_scale;
        }

        {
            let create_tile = self.pressed(ControlName::CreateTile);
            if create_tile || self.pressed(ControlName::DeleteTile)
            {
                let tile_pos = self.pos_to_tile(self.mouse_pos);

                if create_tile
                {
                    self.scenes[self.current_scene][tile_pos] = self.current_tile;
                } else
                {
                    self.scenes[self.current_scene][tile_pos] = Tile::none();
                }
            }
        }

        {
            let canvas = &mut self.window.borrow_mut().canvas;

            canvas.set_draw_color(SdlColor::RGB(0, 0, 0));
            canvas.clear();
        }

        self.draw_scene(&self.scenes[self.current_scene]);

        self.ui.draw();

        let panel = self.tiles_ui.get(&self.tiles_panel);
        let draw_tiles_ui = match self.current_ui
        {
            UiVariant::Tiles =>
            {
                self.tiles_window_animator_open.animate(&mut *panel.borrow_mut());

                true
            },
            UiVariant::Normal =>
            {
                if self.tiles_window_animator_close.is_playing()
                {
                    self.tiles_window_animator_close.animate(&mut *panel.borrow_mut());

                    true
                } else
                {
                    false
                }
            }
        };

        if draw_tiles_ui
        {
            self.tiles_ui.draw();
        }

        self.window.borrow_mut().canvas.present();

        true
    }

    fn set_control(&mut self, control: Keybind, state: bool)
    {
        if let Some((_, control)) = self.keybinds.iter().find(|(k, _)|
        {
            *k == control
        })
        {
            self.controls[*control as usize] = state;
        }
    }

    fn on_event(&mut self, event: Event) -> bool
    {
        match event
        {
            Event::Quit{..} => return false,
            Event::KeyDown{keycode: Some(key), ..} =>
            {
                self.set_control(Keybind::Keyboard(key), true);
            },
            Event::KeyUp{keycode: Some(key), ..} =>
            {
                self.set_control(Keybind::Keyboard(key), false);
            },
            Event::MouseMotion{x, y, ..} =>
            {
                self.mouse_pos = Point2::new(x, y);
            },
            Event::MouseButtonDown{which: button, x, y, ..} =>
            {
                let window_size = self.window_size.map(|x| x as f32);

                let mut pos = Point2::new(x as f32, y as f32) / window_size;
                pos.y = 1.0 - pos.y;

                // thats kinda cool i think thats a cool way to use pattern matching
                if let (0, Some(ui_event)) = (button, self.ui.click(pos))
                {
                    let id = ui_event.element_id;

                    if id == self.next_scene_button
                    {
                        self.current_scene += 1;

                        self.print_current_scene();
                    } else if id == self.prev_scene_button
                    {
                        // yea im not crashing my computer again
                        self.current_scene = self.current_scene.saturating_sub(1);

                        self.print_current_scene();
                    } else if id == self.current_tile_button
                    {
                        self.current_ui = match self.current_ui
                        {
                            UiVariant::Normal =>
                            {
                                self.tiles_window_animator_open.reset();

                                UiVariant::Tiles
                            },
                            UiVariant::Tiles =>
                            {
                                self.tiles_window_animator_close.reset();

                                UiVariant::Normal
                            }
                        };
                    } else
                    {
                        panic!("unhandled element id: {:?}", id)
                    }

                    return true;
                }

                match self.current_ui
                {
                    UiVariant::Tiles =>
                    {
                        if let (0, Some(ui_event)) = (button, self.tiles_ui.click(pos))
                        {
                            let id = ui_event.element_id;

                            if let Some(tile_id) = self.tile_buttons.iter()
                                .position(|element| *element == id)
                            {
                                let tile = Tile::new(tile_id);

                                self.current_tile = tile;

                                self.ensure_current_tile();
                            } else
                            {
                                panic!("cant find button with id: {:?}", id);
                            }
                        }

                        return true;
                    },
                    UiVariant::Normal => ()
                }

                self.set_control(Keybind::Mouse(button), true);
            },
            Event::MouseButtonUp{which: button, ..} =>
            {
                self.set_control(Keybind::Mouse(button), false);
            },
            _ => ()
        }

        true
    }

    fn draw_scene(&self, scene: &Scene)
    {
        for (pos, tile) in scene.iter()
        {
            if tile.is_none()
            {
                continue;
            }

            let size = Point2::repeat(1.0 / self.camera.height);

            let mut pos = self.pos_to_view(pos);
            pos.y = 1.0 - pos.y - size.y;

            let texture_id = self.assets.borrow().tile_texture_id(*tile);

            let mut window = self.window.borrow_mut();

            let assets = self.assets.borrow();
            let texture = assets.texture(texture_id);

            let window_size = self.window_size.map(|x| x as f32);

            let scaled_pos = (pos * window_size).map(|x| x.floor() as i32);

            // u would think that ceil would work but nope
            let scaled_size = (size * window_size).map(|x| x as u32 + 1);

            let x = scaled_pos.x;
            let y = scaled_pos.y;
            let width = scaled_size.x;
            let height = scaled_size.y;

            window.canvas.copy(&texture, None, Rect::new(x, y, width, height))
                .unwrap();
        }
    }

    fn pos_to_tile(&self, pos: Point2<i32>) -> Point2<i32>
    {
        let mut pos = pos.map(|x| x as f32) / self.window_size.map(|x| x as f32);
        pos.y = 1.0 - pos.y;

        let scaled_pos = self.camera.pos / self.camera.height as f32;

        let f_pos = (pos + scaled_pos - 0.5) * self.camera.height as f32;

        f_pos.map(|x| x.floor() as i32)
    }

    fn pos_to_view(&self, pos: Point2<i32>) -> Point2<f32>
    {
        let pos = pos.map(|x| x as f32) / self.camera.height as f32;

        pos - (self.camera.pos / self.camera.height as f32) + 0.5
    }

    fn print_current_scene(&self)
    {
        println!("current scene: {}", self.current_scene);
    }

    fn pressed(&self, control: ControlName) -> bool
    {
        self.controls[control as usize]
    }
}

fn main()
{
    let window_size = Point2{x: 640, y: 480};

    let window = Rc::new(RefCell::new(GameWindow::new(window_size)));

    let mut tiles_amount = 0;

    {
        let window = window.borrow_mut();
        let mut assets = window.assets.borrow_mut();

        fs::read_dir("tiles").unwrap().into_iter().inspect(|_| tiles_amount += 1)
            .zip(iter::repeat(true))
            .chain(fs::read_dir("ui").unwrap().into_iter().zip(iter::repeat(false)))
            .map(|(entry, is_tile)| (entry.unwrap(), is_tile))
            .for_each(|(entry, is_tile)|
            {
                let path = entry.path();

                if is_tile
                {
                    assets.add_tile(path);
                } else
                {
                    assets.add_texture(path);
                }
            });
    }

    let game = Game::new(window_size.map(|x| x as usize), window, tiles_amount);

    game.run();
}
