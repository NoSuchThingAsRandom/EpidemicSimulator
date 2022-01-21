/*
 * Epidemic Simulation Using Census Data (ESUCD)
 * Copyright (c)  2022. Sam Ralph
 *
 * This file is part of ESUCD.
 *
 * ESUCD is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 3 of the License.
 *
 * ESUCD is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with ESUCD.  If not, see <https://www.gnu.org/licenses/>.
 *
 */

use std::fmt::Display;

use geo::prelude::Intersects;
use geo_types::CoordNum;
use log::{trace, warn};

pub const MAX_DEPTH: u8 = 20;
pub const MIN_BOUNDARY_SIZE: usize = 100;

enum Child<T: Clone, U: CoordNum> {
    Quad { children: Box<[QuadTree<T, U>; 4]> },
    Items { items: Items<T, U> },
}

#[derive(Clone)]
pub struct Items<T: Clone, U: CoordNum> {
    items: Vec<(T, geo_types::Rect<U>)>,
}

impl<T: Clone, U: CoordNum> Items<T, U> {
    pub fn add(&mut self, item: T, bounding_box: geo_types::Rect<U>) {
        self.items.push((item, bounding_box))
    }
    pub fn get_items(&self, bounding_box: &geo_types::Rect<U>) -> Vec<&T> {
        self.items.iter().filter_map(|(item, boundary)| {
            if boundary.intersects(bounding_box) {
                Some(item)
            } else {
                None
            }
        }).collect()
    }
    pub fn get_items_mut(&mut self, bounding_box: &geo_types::Rect<U>) -> Vec<&mut T> {
        self.items.iter_mut().filter_map(|(item, boundary)| {
            if boundary.intersects(bounding_box) {
                Some(item)
            } else {
                None
            }
        }).collect()
    }
    pub fn size(&self) -> usize {
        self.items.len()
    }
    pub fn destroy(self) -> Vec<(T, geo_types::Rect<U>)> {
        self.items
    }
}

impl<T: Clone, U: CoordNum> Default for Items<T, U> {
    fn default() -> Self {
        Items { items: Vec::new() }
    }
}


pub struct QuadTree<T: Clone, U: CoordNum> {
    depth: u8,
    max_items_per_quad: usize,
    child: Child<T, U>,
    boundary: geo_types::Rect<U>,
}

impl<'a, T: Clone, U: CoordNum + Display> QuadTree<T, U> {
    pub fn with_size(width: U, height: U, initial_depth: u8, max_items_per_quad: usize) -> QuadTree<T, U> {
        let two = U::one() + U::one();
        let bottom_left = QuadTree::with_boundary(U::zero(), width / two, U::zero(), height / two, 1, initial_depth, max_items_per_quad);
        let bottom_right = QuadTree::with_boundary(width / two, width, U::zero(), height / two, 1, initial_depth, max_items_per_quad);
        let top_left = QuadTree::with_boundary(U::zero(), width / two, height / two, height, 1, initial_depth, max_items_per_quad);
        let top_right = QuadTree::with_boundary(width / two, width, height / two, height, 1, initial_depth, max_items_per_quad);
        let children = Box::new([bottom_left, bottom_right, top_left, top_right]);
        QuadTree {
            depth: 0,
            max_items_per_quad,
            child: Child::Quad { children },
            boundary: geo_types::Rect::new((U::zero(), U::zero()), (width, height)),
        }
    }

    /// Builds a new Quadtree with a child of Items
    fn new_item_child(x_min: U,
                      x_max: U,
                      y_min: U,
                      y_max: U, depth: u8,
                      max_items_per_quad: usize) -> QuadTree<T, U> {
        QuadTree {
            depth,
            max_items_per_quad,
            child: Child::Items { items: Items::default() },
            boundary: geo_types::Rect::new((x_min, y_min), (x_max, y_max)),
        }
    }
    /// Builds a new Quadtree, with 4 children
    fn build_single_layer_quadtree(x_min: U,
                                   x_max: U,
                                   y_min: U,
                                   y_max: U, depth: u8,
                                   max_items_per_quad: usize) -> Child<T, U> {
        let two = U::one() + U::one();
        let width = x_max - x_min;
        let height = y_max - y_min;

        let bottom_left = QuadTree::new_item_child(x_min, x_min + (width / two), y_min, y_min + (height / two), depth + 1, max_items_per_quad);
        let bottom_right = QuadTree::new_item_child(x_min + (width / two), x_max, y_min, y_min + (height / two), depth + 1, max_items_per_quad);

        let top_left = QuadTree::new_item_child(x_min, x_min + (width / two), y_min + (height / two), y_max, depth + 1, max_items_per_quad);
        let top_right = QuadTree::new_item_child(x_min + (width / two), x_max, y_min + (height / two), y_max, depth + 1, max_items_per_quad);

        let children = Box::new([bottom_left, bottom_right, top_left, top_right]);
        Child::Quad { children }
    }

    /// Builds a new quadtree with a depth of `initial_depth`
    fn with_boundary(x_min: U,
                     x_max: U,
                     y_min: U,
                     y_max: U, current_depth: u8, initial_depth: u8, max_items_per_quad: usize) -> QuadTree<T, U> {
        assert!(x_min < x_max, "X min ({}) must be smaller than X max ({})", x_min, x_max);
        assert!(y_min < y_max, "Y min ({}) must be smaller than Y max ({})", y_min, y_max);

        let child = if current_depth < initial_depth + 1 && (x_max - x_min) > U::from(MIN_BOUNDARY_SIZE).unwrap() && (y_max - y_min) > U::from(MIN_BOUNDARY_SIZE).unwrap() {
            let two = U::one() + U::one();
            let width = x_max - x_min;
            let height = y_max - y_min;

            let bottom_left = QuadTree::with_boundary(x_min, x_min + (width / two), y_min, y_min + (height / two), current_depth + 1, initial_depth, max_items_per_quad);
            let bottom_right = QuadTree::with_boundary(x_min + (width / two), x_max, y_min, y_min + (height / two), current_depth + 1, initial_depth, max_items_per_quad);

            let top_left = QuadTree::with_boundary(x_min, x_min + (width / two), y_min + (height / two), y_max, current_depth + 1, initial_depth, max_items_per_quad);
            let top_right = QuadTree::with_boundary(x_min + (width / two), x_max, y_min + (height / two), y_max, current_depth + 1, initial_depth, max_items_per_quad);


            let children = Box::new([bottom_left, bottom_right, top_left, top_right]);
            Child::Quad { children }
        } else {
            Child::Items { items: Items::default() }
        };
        QuadTree {
            depth: current_depth,
            max_items_per_quad,
            child,
            boundary: geo_types::Rect::new((x_min, y_min), (x_max, y_max)),
        }
    }
    pub fn add_item(&mut self, item: T, bounding_box: geo_types::Rect<U>) -> bool {
        return match &mut self.child {
            Child::Quad { children } => {
                let mut added = false;
                for child in &mut children.iter_mut() {
                    if child.contains(&bounding_box) && child.add_item(item.clone(), bounding_box) {
                        added = true;
                    }
                }
                added
            }
            Child::Items { items } => {
                items.add(item, bounding_box);
                if items.size() > self.max_items_per_quad {
                    self.rebuild()
                }
                true
            }
        };
    }

    /// If the Items struct has too many entries, create a sub quad grid to increase indexing performance
    pub fn rebuild(&mut self) {
        trace!("Commencing rebuild at depth: {}",self.depth);
        let items = match &self.child {
            Child::Quad { .. } => {
                warn!("Can't destroy sub quadtrees!");
                return;
            }
            Child::Items { items } => {
                (items.clone()).destroy()
            }
        };
        self.child = QuadTree::build_single_layer_quadtree(self.boundary.min().x, self.boundary.max().x, self.boundary.min().y, self.boundary.max().y, self.depth, self.max_items_per_quad);
        for (item, boundary) in items {
            assert!(self.add_item(item, boundary), "Failed to build lower layer")
        }
    }
    pub fn contains(&self, other: &geo_types::Rect<U>) -> bool {
        self.boundary.intersects(other)
    }
    pub fn get_items(&self, bounding_box: &geo_types::Rect<U>) -> Vec<&T> {
        match &self.child {
            Child::Quad { children } => {
                children.iter().filter_map(|child| if child.contains(&bounding_box) { Some(child.get_items(bounding_box)) } else { None }).flatten().collect()
            }
            Child::Items { items } => {
                items.get_items(bounding_box)
            }
        }
    }
    pub fn get_items_mut(&mut self, bounding_box: &geo_types::Rect<U>) -> Vec<&mut T> {
        match &mut self.child {
            Child::Quad { children } => {
                children.iter_mut().filter_map(|child| if child.contains(&bounding_box) { Some(child.get_items_mut(bounding_box)) } else { None }).flatten().collect()
            }
            Child::Items { items } => {
                items.get_items_mut(bounding_box)
            }
        }
    }
}