use std::path::Path;

use crate::Point2;


pub struct Image
{
    data: Vec<u8>,
    size: Point2<usize>,
    bpp: usize
}

impl Image
{
    pub fn load(path: impl AsRef<Path>) -> Self
    {
        let image = image::open(path).unwrap().into_rgba8();

        Self{
            size: Point2::new(image.width() as usize, image.height() as usize),
            data: image.into_raw(),
            bpp: 4
        }
    }

    pub fn data(&self) -> &[u8]
    {
        &self.data
    }

    pub fn size(&self) -> &Point2<usize>
    {
        &self.size
    }

    pub fn bpp(&self) -> usize
    {
        self.bpp
    }

    pub fn bytes_row(&self) -> usize
    {
        self.bpp * self.size.x
    }
}
