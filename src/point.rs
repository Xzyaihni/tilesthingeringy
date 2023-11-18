use std::{
    fmt::Debug,
    ops::{
        Add,
        Sub,
        Mul,
        Div,
        AddAssign,
        SubAssign,
        MulAssign,
        DivAssign,
        Neg
    }
};

use sdl2::rect::Point as SDLPoint;


#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Point2<T>
{
    pub x: T,
    pub y: T
}

impl<T> Point2<T>
{
    pub fn new(x: T, y: T) -> Self
    {
        Self{x, y}
    }

    pub fn repeat(value: T) -> Self
    where
        T: Clone
    {
        Self{x: value.clone(), y: value}
    }

    pub fn cast<U: TryFrom<T>>(self) -> Point2<U>
    where
        <U as TryFrom<T>>::Error: Debug
    {
        Point2{x: self.x.try_into().unwrap(), y: self.y.try_into().unwrap()}
    }

    pub fn zip<U>(self, other: Point2<U>) -> Point2<(T, U)>
    {
        Point2{x: (self.x, other.x), y: (self.y, other.y)}
    }

    pub fn map<F, U>(self, mut f: F) -> Point2<U>
    where
        F: FnMut(T) -> U
    {
        Point2{x: f(self.x), y: f(self.y)}
    }
}

impl Point2<i32>
{
    pub fn abs(self) -> Self
    {
        Self{
            x: self.x.abs(),
            y: self.y.abs()
        }
    }
}

impl Point2<f64>
{
    pub fn rotate(self, rotation: f64) -> Self
    {
        let (r_sin, r_cos) = rotation.sin_cos();

        Point2{
            x: r_cos * self.x + r_sin * self.y,
            y: r_cos * self.y - r_sin * self.x
        }
    }

    pub fn abs(self) -> Self
    {
        Self{
            x: self.x.abs(),
            y: self.y.abs()
        }
    }
}

impl Into<SDLPoint> for Point2<usize>
{
    fn into(self) -> SDLPoint
    {
        SDLPoint::new(self.x as i32, self.y as i32)
    }
}

macro_rules! op_impl
{
    ($op_trait:ident, $op_fn:ident) =>
    {
        impl<T: $op_trait<Output=T>> $op_trait<Point2<T>> for Point2<T>
        {
            type Output = Point2<T>;

            fn $op_fn(self, rhs: Point2<T>) -> Self::Output
            {
                Point2{
                    x: self.x.$op_fn(rhs.x),
                    y: self.y.$op_fn(rhs.y)
                }
            }
        }

        impl<T: $op_trait<Output=T> + Clone> $op_trait<Point2<T>> for &Point2<T>
        {
            type Output = Point2<T>;

            fn $op_fn(self, rhs: Point2<T>) -> Self::Output
            {
                Point2{
                    x: self.x.clone().$op_fn(rhs.x),
                    y: self.y.clone().$op_fn(rhs.y)
                }
            }
        }
    }
}

macro_rules! op_impl_assign
{
    ($op_trait:ident, $op_fn:ident) =>
    {
        impl<T: $op_trait> $op_trait<Point2<T>> for Point2<T>
        {
            fn $op_fn(&mut self, rhs: Point2<T>)
            {
                self.x.$op_fn(rhs.x);
                self.y.$op_fn(rhs.y);
            }
        }
    }
}

macro_rules! op_impl_scalar
{
    ($op_trait:ident, $op_fn:ident) =>
    {
        impl<T: $op_trait<Output=T> + Clone> $op_trait<T> for Point2<T>
        {
            type Output = Point2<T>;

            fn $op_fn(self, rhs: T) -> Self::Output
            {
                Point2{
                    x: self.x.$op_fn(rhs.clone()),
                    y: self.y.$op_fn(rhs)
                }
            }
        }

        impl<T: $op_trait<Output=T> + Clone> $op_trait<T> for &Point2<T>
        {
            type Output = Point2<T>;

            fn $op_fn(self, rhs: T) -> Self::Output
            {
                Point2{
                    x: self.x.clone().$op_fn(rhs.clone()),
                    y: self.y.clone().$op_fn(rhs)
                }
            }
        }
    }
}

impl<T: Neg<Output=T>> Neg for Point2<T>
{
    type Output = Self;

    fn neg(self) -> Self::Output
    {
        Point2{
            x: -self.x,
            y: -self.y
        }
    }
}

op_impl!{Add, add}
op_impl!{Sub, sub}
op_impl!{Mul, mul}
op_impl!{Div, div}

op_impl_assign!{AddAssign, add_assign}
op_impl_assign!{SubAssign, sub_assign}
op_impl_assign!{MulAssign, mul_assign}
op_impl_assign!{DivAssign, div_assign}

op_impl_scalar!{Add, add}
op_impl_scalar!{Sub, sub}
op_impl_scalar!{Mul, mul}
op_impl_scalar!{Div, div}
