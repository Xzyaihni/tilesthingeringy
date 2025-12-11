use std::{
    rc::Rc,
    cell::RefCell,
    ops::ControlFlow
};

use sdl2::rect::Rect;

use crate::{Point2, GameWindow, Assets, TextureId, animator::Animatable};


// i could just store the children in a vec but this is much cooler
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementId
{
    id: usize,
    child: Option<Box<ElementId>>
}

impl ElementId
{
    pub fn new(id: usize) -> Self
    {
        Self{id, child: None}
    }

    pub fn push(&self, child_id: usize) -> Self
    {
        let mut element = self.clone();

        element.set_tail(child_id);

        element
    }

    pub fn set_tail(&mut self, child_id: usize)
    {
        if let Some(child_element) = self.child.as_mut()
        {
            child_element.set_tail(child_id)
        } else
        {
            self.child = Some(Box::new(Self::new(child_id)));
        }
    }
}

pub struct UiEvent
{
    pub element_id: ElementId
}

pub enum UiElementType
{
    Panel,
    Button
}

pub struct UiElement
{
    pub kind: UiElementType,
    pub pos: Point2<f32>,
    pub size: Point2<f32>,
    pub texture: TextureId
}

struct UiElementGlobal
{
    inner: UiElement,
    global_size: Point2<f32>,
    global_pos: Point2<f32>,
}

impl UiElementGlobal
{
    pub fn intersects(&self, pos: Point2<f32>) -> bool
    {
        (self.global_pos.x..=(self.global_pos.x + self.global_size.x)).contains(&pos.x)
            && (self.global_pos.y..=(self.global_pos.y + self.global_size.y)).contains(&pos.y)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UiAnimatableId
{
    ScaleX,
    ScaleY,
    PositionX,
    PositionY
}

pub struct UiElementInner
{
    parent: Option<(usize, Rc<RefCell<Self>>)>,
    element: UiElementGlobal,
    children: Vec<Rc<RefCell<Self>>>
}

impl UiElementInner
{
    pub fn texture(&mut self) -> &mut TextureId
    {
        &mut self.element.inner.texture
    }

    fn new_parent(element: UiElement) -> Rc<RefCell<Self>>
    {
        Self::new_inner(None, element)
    }

    fn new_child(parent: Rc<RefCell<Self>>, id: usize, element: UiElement) -> Rc<RefCell<Self>>
    {
        Self::new_inner(Some((id, parent)), element)
    }

    fn new_inner(
        parent: Option<(usize, Rc<RefCell<Self>>)>,
        element: UiElement
    ) -> Rc<RefCell<Self>>
    {
        Rc::new(RefCell::new(Self{
            parent,
            element: UiElementGlobal{

                global_size: element.size,
                global_pos: element.pos,
                inner: element
            },
            children: Vec::new()
        }))
    }

    fn push(this: &Rc<RefCell<Self>>, element: UiElement) -> usize
    {
        let parent = this.clone();

        let mut this = this.borrow_mut();

        let id = this.children.len();

        this.children.push(Self::new_child(parent, id, element));

        this.update_child(id);

        id
    }

    fn update_child(&mut self, id: usize)
    {
        let mut child = self.children[id].borrow_mut();
        let this = &mut self.element;

        {
            let child = &mut child.element;

            child.global_pos = this.global_pos + child.inner.pos * this.global_size;
            child.global_size = child.inner.size * this.global_size;
        }

        child.update_children();
    }

    fn update_children(&mut self)
    {
        for i in 0..self.children.len()
        {
            self.update_child(i);
        }
    }

    fn update(&mut self)
    {
        if let Some((id, parent)) = self.parent.as_ref()
        {
            parent.borrow_mut().update_child(*id);
        } else
        {
            self.element.global_pos = self.element.inner.pos;
            self.element.global_size = self.element.inner.size;

            self.update_children();
        }
    }

    fn get(&self, id: &ElementId) -> Rc<RefCell<Self>>
    {
        let this = &self.children[id.id];

        if let Some(child_id) = id.child.as_ref()
        {
            this.borrow().get(&child_id)
        } else
        {
            this.clone()
        }
    }

    fn try_for_each_element<T, F>(&self, id: ElementId, f: &mut F) -> ControlFlow<T>
    where
        F: FnMut(&ElementId, &UiElementGlobal) -> ControlFlow<T>
    {
        match f(&id, &self.element)
        {
            ControlFlow::Continue(_) => (),
            x => return x
        }

        self.children.iter().enumerate().try_for_each(|(index, child)|
        {
            let id = id.push(index);

            child.borrow().try_for_each_element(id, f)
        })
    }
}

impl Animatable<UiAnimatableId> for UiElementInner
{
    fn set(&mut self, id: &UiAnimatableId, value: f32)
    {
        match id
        {
            UiAnimatableId::ScaleX =>
            {
                self.element.inner.size.x = value;
            },
            UiAnimatableId::ScaleY =>
            {
                self.element.inner.size.y = value;
            },
            UiAnimatableId::PositionX =>
            {
                self.element.inner.pos.x = value;
            },
            UiAnimatableId::PositionY =>
            {
                self.element.inner.pos.y = value;
            }
        }

        self.update();
    }
}

pub struct Ui
{
    window: Rc<RefCell<GameWindow>>,
    assets: Rc<RefCell<Assets>>,
    elements: Vec<Rc<RefCell<UiElementInner>>>
}

impl Ui
{
    pub fn new(window: Rc<RefCell<GameWindow>>, assets: Rc<RefCell<Assets>>) -> Self
    {
        Self{window, assets, elements: Vec::new()}
    }

    pub fn push(&mut self, element: UiElement) -> ElementId
    {
        let id = self.elements.len();

        self.elements.push(UiElementInner::new_parent(element));

        ElementId::new(id)
    }

    pub fn push_child(&mut self, parent_id: &ElementId, element: UiElement) -> ElementId
    {
        let id = UiElementInner::push(&self.get(parent_id), element);

        parent_id.push(id)
    }

    // if i wasnt lazy i wouldnt need to have this be an exact copy of a function above
    pub fn get(&self, id: &ElementId) -> Rc<RefCell<UiElementInner>>
    {
        let this = &self.elements[id.id];

        if let Some(child_id) = id.child.as_ref()
        {
            this.borrow().get(&child_id)
        } else
        {
            this.clone()
        }
    }

    pub fn draw(&self)
    {
        let mut window = self.window.borrow_mut();
        let assets = self.assets.borrow();

        let window_size = window.window_size().map(|x| x as f32);

        self.for_each_element(|_id, element|
        {
            let texture = assets.texture(element.inner.texture);

            let scaled_pos = {
                let mut pos = element.global_pos;

                pos.y = 1.0 - pos.y - element.global_size.y;

                pos * window_size
            }.map(|x| x.round() as i32);

            let scaled_size = (element.global_size * window_size)
                .map(|x| x.round() as u32);

            let x = scaled_pos.x;
            let y = scaled_pos.y;
            let width = scaled_size.x;
            let height = scaled_size.y;

            window.canvas.copy(&texture, None, Rect::new(x, y, width, height))
                .unwrap();
        });
    }

    pub fn click(&self, pos: Point2<f32>) -> Option<UiEvent>
    {
        match self.try_for_each_element(|id, element|
        {
            match element.inner.kind
            {
                UiElementType::Button =>
                {
                    if element.intersects(pos)
                    {
                        return ControlFlow::Break(UiEvent{element_id: id.clone()});
                    }
                },
                UiElementType::Panel => ()
            }

            ControlFlow::Continue(())
        })
        {
            ControlFlow::Break(x) => Some(x),
            ControlFlow::Continue(_) => None
        }
    }

    fn try_for_each_element<T, F>(&self, mut f: F) -> ControlFlow<T>
    where
        F: FnMut(&ElementId, &UiElementGlobal) -> ControlFlow<T>
    {
        self.elements.iter().enumerate().try_for_each(|(index, element)|
        {
            let id = ElementId::new(index);

            element.borrow().try_for_each_element(id, &mut f)
        })
    }

    fn for_each_element<F>(&self, mut f: F)
    where
        F: FnMut(&ElementId, &UiElementGlobal)
    {
        let _ = self.try_for_each_element(|id, element|
        {
            f(id, element);

            ControlFlow::<()>::Continue(())
        });
    }
}
