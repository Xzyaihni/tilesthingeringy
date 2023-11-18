use std::{
    slice::{
        Iter as SliceIter,
        IterMut as SliceIterMut
    },
    iter::Enumerate,
    ops::{Index, IndexMut}
};

use crate::Point2;


#[derive(Debug, Clone)]
pub struct Indexer
{
    size: Point2<usize>
}

impl Indexer
{
    pub fn new(size: Point2<usize>) -> Self
    {
        Self{size}
    }

    pub fn to_index(&self, pos: Point2<usize>) -> usize
    {
        Self::to_index_assoc(self.size, pos)
    }

    pub fn to_index_assoc(size: Point2<usize>, pos: Point2<usize>) -> usize
    {
        pos.y * size.x + pos.x
    }

    pub fn index_to_pos(&self, index: usize) -> Point2<usize>
    {
        Self::index_to_pos_assoc(self.size, index)
    }

    pub fn index_to_pos_assoc(size: Point2<usize>, index: usize) -> Point2<usize>
    {
        Point2{
            x: index % size.x,
            y: index / size.x
        }
    }
}

macro_rules! impl_iter
{
    ($name:ident, $other_iter:ident) =>
    {
        pub struct $name<'a, T>
        {
            data: Enumerate<$other_iter<'a, T>>,
            indexer: Indexer
        }

        impl<'a, T> $name<'a, T>
        {
            #[allow(dead_code)]
            pub fn new(data: $other_iter<'a, T>, indexer: Indexer) -> Self
            {
                Self{data: data.enumerate(), indexer}
            }
        }

        impl<'a, T> Iterator for $name<'a, T>
        {
            type Item = (Point2<usize>, <$other_iter<'a, T> as Iterator>::Item);

            fn next(&mut self) -> Option<Self::Item>
            {
                self.data.next().map(|(index, value)| (self.indexer.index_to_pos(index), value))
            }
        }
    }
}

impl_iter!{Iter, SliceIter}
impl_iter!{IterMut, SliceIterMut}

#[derive(Debug)]
pub struct Container2d<T>
{
    data: Box<[T]>,
    indexer: Indexer,
    size: Point2<usize>
}

impl<T> Container2d<T>
{
    pub fn new(size: Point2<usize>) -> Self
    where
        T: Default
    {
        let data = (0..(size.x * size.y)).map(|_| T::default()).collect();

        let indexer = Indexer::new(size);

        Self{data, indexer, size}
    }

    pub fn size(&self) -> &Point2<usize>
    {
        &self.size
    }

    pub fn iter(&self) -> Iter<T>
    {
        Iter::new(self.data.iter(), self.indexer.clone())
    }

    #[allow(dead_code)]
    pub fn iter_mut(&mut self) -> IterMut<T>
    {
        IterMut::new(self.data.iter_mut(), self.indexer.clone())
    }
}

impl<T> Index<Point2<usize>> for Container2d<T>
{
    type Output = T;

    fn index(&self, index: Point2<usize>) -> &Self::Output
    {
        &self.data[self.indexer.to_index(index)]
    }
}

impl<T> IndexMut<Point2<usize>> for Container2d<T>
{
    fn index_mut(&mut self, index: Point2<usize>) -> &mut Self::Output
    {
        let index = self.indexer.to_index(index);

        &mut self.data[index]
    }
}
