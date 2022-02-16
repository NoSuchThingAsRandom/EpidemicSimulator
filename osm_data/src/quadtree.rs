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
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use geo::prelude::Intersects;
use geo_types::{Coordinate, CoordNum};
use log::{trace, warn};
use serde;
use serde::{Serialize, Serializer};
use serde::ser::SerializeSeq;

pub const MAX_DEPTH: u8 = 20;
pub const MIN_BOUNDARY_SIZE: usize = 100;

/// Center point for a rect ([`geo_types::rect::Rect::center()`] for [`geo_types::CoordNum`], as geo_types only implement it for [`geo_types::CoordFloat`]
pub fn center<T: geo_types::CoordNum>(rect: geo_types::Rect<T>) -> Coordinate<T> {
    let two = T::one() + T::one();
    (
        (rect.max().x + rect.min().x) / two,
        (rect.max().y + rect.min().y) / two,
    )
        .into()
}

pub fn compare_geo_coord_nums<T: geo_types::CoordNum>(a: T, b: T) -> Ordering {
    if b < a {
        Ordering::Less
    } else if a == b {
        Ordering::Equal
    } else {
        Ordering::Greater
    }
}

/// Who doesn't like writing their own absolute function?
pub fn coord_num_abs<T: geo_types::CoordNum>(mut a: T) -> T {
    let mut counter: T = T::zero();
    while a < T::zero() {
        a = a + T::one();
        counter = counter + T::one();
    }
    println!("Reducing counter");
    while counter > T::zero() {
        a = a + T::one();
        counter = counter - T::one();
    }
    a
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::quadtree::{compare_geo_coord_nums, coord_num_abs};

    #[test]
    fn abs_test() {
        let coord = geo_types::Coordinate::from((500, 500));
        assert_eq!(coord.x, coord_num_abs(coord.x));

        let coord = geo_types::Coordinate::from((-500, 500));
        assert_eq!(coord.x, -coord_num_abs(coord.x));
    }

    #[test]
    fn compare_test() {
        let coord = geo_types::Coordinate::from((500, 200));
        assert_eq!(compare_geo_coord_nums(coord.x, coord.y), Ordering::Less);
        assert_eq!(compare_geo_coord_nums(coord.x, coord.x), Ordering::Equal);
        assert_eq!(compare_geo_coord_nums(coord.y, coord.x), Ordering::Greater);

        let coord = geo_types::Coordinate::from((-500, -700));
        assert_eq!(compare_geo_coord_nums(coord.x, coord.y), Ordering::Less);
        assert_eq!(compare_geo_coord_nums(coord.x, coord.x), Ordering::Equal);
        assert_eq!(compare_geo_coord_nums(coord.y, coord.x), Ordering::Greater);
    }
}

/// Returns the square abs function as abs is not supported for CoordNum
pub fn manhattan_distance<T: geo_types::CoordNum>(a: geo_types::Coordinate<T>, b: geo_types::Coordinate<T>) -> T {
    // TODO This is fucking cursed
    let x = a.x - b.x;
    let x: isize = format!("{:?}", x).parse().unwrap();// as isize;
    let y = a.y - b.y;
    let y: isize = format!("{:?}", y).parse().unwrap();// as isize;
    T::from(x.abs() + y.abs()).unwrap()
}

#[derive(Serialize)]
enum Child<T: Clone, U: CoordNum> {
    Quad { children: Box<[QuadTree<T, U>; 4]> },
    Items { items: Items<T, U> },
}

impl<T: Clone + Debug, U: CoordNum> Display for Child<T, U> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Child::Quad { children } => {
                write!(f, "\n")?;
                for child in children.iter() {
                    write!(f, "\t -> {}\n", child)?;
                }
            }
            Child::Items { items } => {
                write!(f, "\t -> {:?}", items.items)?;
            }
        }
        write!(f, "")
    }
}


#[derive(Clone)]
pub struct Items<T: Clone, U: CoordNum> {
    items: Vec<(T, geo_types::Rect<U>)>,
}

impl<T: Clone, U: CoordNum + Serialize> Serialize for Items<T, U> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.items.len()))?;
        for (_, rect) in &self.items {
            seq.serialize_element(rect)?;
        }
        seq.end()
    }
}


impl<T: Clone, U: CoordNum> Items<T, U> {
    /// Adds a new item to the list, with the given bounding area
    pub fn add(&mut self, item: T, bounding_box: geo_types::Rect<U>) {
        self.items.push((item, bounding_box))
    }
    /// Returns the items[`T`] and distance [`U`] to the center point of the given bounding box
    ///
    /// The items are sorted in increasing distance from the bounding box
    pub fn get_items_sorted(&self, bounding_box: geo_types::Rect<U>) -> Vec<(&T, U)> {
        let search_value_center = center(bounding_box);
        let mut items: Vec<(&T, U)> = self.items.iter().map(|(item, boundary)| {
            if boundary.intersects(&bounding_box) {
                (item, U::zero())
            } else {
                let distance = manhattan_distance(search_value_center, center(*boundary));
                (item, distance)
            }
        }).collect();
        items.sort_by(|(_, a_distance), (_, b_distance)| compare_geo_coord_nums(*a_distance, *b_distance));
        items
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


#[derive(Serialize)]
pub struct QuadTree<T: Clone, U: CoordNum> {
    depth: u8,
    #[serde(skip)]
    max_items_per_quad: usize,
    child: Child<T, U>,
    boundary: geo_types::Rect<U>,
}

impl<'a, T: Clone + Eq + Hash, U: CoordNum + Display> QuadTree<T, U> {
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
    /// Returns the top [`MAX_ITEMS_RETURNED`] closest items to the bounding box
    pub fn get_multiple_items(&'a self, bounding_box: geo_types::Rect<U>) -> Vec<(&T, U)> {//Box<dyn Iterator<Item=(&T, U)> + 'a> {
        match &self.child {
            Child::Quad { children } => {
                let mut items = HashMap::with_capacity(MAX_ITEMS_RETURNED);
                let mut used_children = [false; 4];

                // Get all elements from the direct child containing the box
                for (index, child) in children.iter().enumerate() {
                    if child.contains(&bounding_box) {
                        let new_items = child.get_multiple_items(bounding_box);
                        for (id, container) in new_items {
                            items.insert(id, container);
                        }
                        used_children[index] = true;
                    }
                }
                // Fill up the items buffer if it is less than MAX_ITEMS_RETURNED
                for (index, is_used) in used_children.iter().enumerate() {
                    if items.len() >= MAX_ITEMS_RETURNED {
                        break;
                    }
                    if !is_used {
                        let child = &children[index];
                        let new_items = child.get_multiple_items(bounding_box);
                        for (id, container) in new_items {
                            items.insert(id, container);
                        }
                    }
                }
                let mut items: Vec<(&T, U)> = items.into_iter().collect();
                items.truncate(MAX_ITEMS_RETURNED);
                // TODO Maybe we don't need this sort?
                items.sort_by(|(_, a), (_, b)| compare_geo_coord_nums(*a, *b));
                items
            }
            Child::Items { items } => {
                items.get_items_sorted(bounding_box)
            }
        }
    }
    /// Returns references to all Items that are encapsulated by the given bounding box
    pub fn get_items(&self, bounding_box: geo_types::Rect<U>) -> Vec<&T> {
        match &self.child {
            Child::Quad { children } => {
                children.iter().filter_map(|child| if child.contains(&bounding_box) { Some(child.get_items(bounding_box)) } else { None }).flatten().collect()
            }
            Child::Items { items } => {
                items.get_items(&bounding_box)
            }
        }
    }
    /// Returns mutable references to all items that are encapsulated by the given bounding box
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

impl<T: Clone + Debug, U: CoordNum> Display for QuadTree<T, U> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:->\t{}", self.depth, self.child)
    }
}

pub const MAX_ITEMS_RETURNED: usize = 200;