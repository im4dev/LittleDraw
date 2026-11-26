//! Cartesian planes for math classes: one drag lays out a grid, two axes
//! with arrowheads, numbered ticks and the `x`/`y` labels, all as plain
//! Excalidraw elements (one grouped line, two arrows, texts) so the file
//! still opens in Excalidraw.
//!
//! The grid line carries `customData.cartesianPlane` with its cell counts,
//! so lines and arrows drawn later can snap to its intersections even
//! after the plane was moved or resized.

use kurbo::{Point, Rect, Vec2};
use serde_json::{json, Value};

use crate::element::{Element, Kind, StrokeStyle};
use crate::text;

/// Side of one unit, in scene pixels, when the plane is created.
pub const UNIT: f64 = 40.0;
/// Color of the grid lines; the axes and labels use the stroke color.
const GRID_COLOR: &str = "#ced4da";
const LABEL_SIZE: f64 = 16.0;
/// How far the axes run past the grid, in units.
const AXIS_OVERHANG: f64 = 1.1;
/// Gap between a tick and its number, in scene pixels.
const LABEL_GAP: f64 = 4.0;
/// Fraction of a unit within which a point snaps to an intersection.
const SNAP_REACH: f64 = 0.3;
const DATA_KEY: &str = "cartesianPlane";

/// A plane `columns`×`rows` units in size whose origin sits `origin_column`
/// units from the left and `origin_row` units from the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub top_left: Point,
    pub unit: f64,
    pub columns: u32,
    pub rows: u32,
    pub origin_column: u32,
    pub origin_row: u32,
}

impl Plane {
    /// The plane a drag from `from` to `to` lays out, in whole units.
    /// Normally four quadrants with the origin in the middle; with
    /// `first_quadrant`, only positive values, origin at the bottom left.
    pub fn from_drag(from: Point, to: Point, first_quadrant: bool) -> Plane {
        let d = to - from;
        let count = |len: f64| {
            if first_quadrant {
                ((len.abs() / UNIT).round() as u32).max(1)
            } else {
                // Even counts keep the two halves the same size.
                ((len.abs() / UNIT / 2.0).round() as u32).max(1) * 2
            }
        };
        let (columns, rows) = (count(d.x), count(d.y));
        let (w, h) = (f64::from(columns) * UNIT, f64::from(rows) * UNIT);
        let x = if d.x < 0.0 { from.x - w } else { from.x };
        let y = if d.y < 0.0 { from.y - h } else { from.y };
        let (origin_column, origin_row) = if first_quadrant { (0, rows) } else { (columns / 2, rows / 2) };
        Plane { top_left: Point::new(x, y), unit: UNIT, columns, rows, origin_column, origin_row }
    }

    pub fn origin(&self) -> Point {
        self.top_left + Vec2::new(f64::from(self.origin_column), f64::from(self.origin_row)) * self.unit
    }

    fn bounds(&self) -> Rect {
        Rect::from_origin_size(self.top_left, (f64::from(self.columns) * self.unit, f64::from(self.rows) * self.unit))
    }

    /// The plane a grid element made by [`Plane::elements`] stands for,
    /// wherever it was moved or however it was resized since.
    pub fn of(el: &Element) -> Option<Plane> {
        if el.is_deleted || el.kind != Kind::Line || el.angle != 0.0 {
            return None;
        }
        let data = el.extra.get("customData")?.get(DATA_KEY)?;
        let get = |key: &str| data.get(key).and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok());
        let (columns, rows) = (get("columns")?.max(1), get("rows")?.max(1));
        let b = el.local_bounds();
        let unit = b.width() / f64::from(columns);
        // Resized unevenly: the cells are no longer square.
        if unit <= 0.0 || (b.height() / f64::from(rows) - unit).abs() > 0.5 {
            return None;
        }
        Some(Plane {
            top_left: Point::new(el.x + b.x0, el.y + b.y0),
            unit,
            columns,
            rows,
            origin_column: get("originColumn")?.min(columns),
            origin_row: get("originRow")?.min(rows),
        })
    }

    /// The grid intersection near `p`, if `p` is on the plane and close
    /// enough to one.
    pub fn snap(&self, p: Point) -> Option<Point> {
        let reach = self.unit * SNAP_REACH;
        if !self.bounds().inflate(reach, reach).contains(p) {
            return None;
        }
        let o = self.origin();
        let cell = ((p - o) / self.unit).round();
        let q = o + cell * self.unit;
        (q.distance(p) <= reach).then_some(q)
    }

    /// The plane's elements, without ids, seeds or group: the grid line
    /// first, then the axes and the labels. `color` and `font_family` are
    /// those of the axes and numbers.
    pub fn elements(&self, color: &str, font_family: i64) -> Vec<Element> {
        let b = self.bounds();
        let o = self.origin();
        let u = self.unit;
        let base = Element {
            stroke_color: color.to_owned(),
            roughness: 0.0,
            stroke_style: StrokeStyle::Solid,
            ..Element::default()
        };
        let mut out = Vec::new();

        // The whole grid as one line snaking down the columns and across
        // the rows; the turns run along the border, which is drawn anyway.
        let mut pts = Vec::new();
        for i in 0..=self.columns {
            let x = f64::from(i) * u;
            let (a, z) = if i % 2 == 0 { (0.0, b.height()) } else { (b.height(), 0.0) };
            pts.push([x, a]);
            pts.push([x, z]);
        }
        // The columns end in the right corner, bottom or top; the rows start
        // there, heading left.
        let ends_at_bottom = self.columns % 2 == 0;
        for k in 0..=self.rows {
            let j = if ends_at_bottom { self.rows - k } else { k };
            let y = f64::from(j) * u;
            let (a, z) = if k % 2 == 0 { (b.width(), 0.0) } else { (0.0, b.width()) };
            pts.push([a, y]);
            pts.push([z, y]);
        }
        pts.dedup();
        let mut custom = serde_json::Map::new();
        custom.insert(
            DATA_KEY.to_owned(),
            json!({
                "columns": self.columns,
                "rows": self.rows,
                "originColumn": self.origin_column,
                "originRow": self.origin_row,
            }),
        );
        let mut grid = Element {
            kind: Kind::Line,
            x: b.x0,
            y: b.y0,
            width: b.width(),
            height: b.height(),
            stroke_color: GRID_COLOR.to_owned(),
            stroke_width: 1.0,
            points: pts,
            ..base.clone()
        };
        grid.extra.insert("customData".to_owned(), Value::Object(custom));
        out.push(grid);

        // Axes: both ends get a head where the plane goes on past the grid.
        let over = AXIS_OVERHANG * u;
        let first_quadrant = self.origin_column == 0 && self.origin_row == self.rows;
        let axis = |from: Point, to: Point, both: bool| Element {
            kind: Kind::Arrow,
            x: from.x,
            y: from.y,
            width: (to.x - from.x).abs(),
            height: (to.y - from.y).abs(),
            stroke_width: 2.0,
            points: vec![[0.0, 0.0], [to.x - from.x, to.y - from.y]],
            start_arrowhead: both.then(|| "arrow".to_owned()),
            end_arrowhead: Some("arrow".to_owned()),
            ..base.clone()
        };
        let (x_from, y_from) = if first_quadrant { (o.x, o.y) } else { (b.x0 - over, b.y1 + over) };
        out.push(axis(Point::new(x_from, o.y), Point::new(b.x1 + over, o.y), !first_quadrant));
        out.push(axis(Point::new(o.x, y_from), Point::new(o.x, b.y0 - over), !first_quadrant));

        let line_height = text::metrics(font_family).line_height;
        let label = |s: &str, size: f64, anchor: Point, align: Vec2| {
            let (w, h) = text::measure(s, font_family, size, line_height);
            Element {
                kind: Kind::Text,
                // `align` is where `anchor` sits on the label, from (0, 0) at
                // its top left to (1, 1) at its bottom right.
                x: anchor.x - w * align.x,
                y: anchor.y - h * align.y,
                width: w,
                height: h,
                text: Some(s.to_owned()),
                original_text: Some(s.to_owned()),
                font_size: Some(size),
                font_family: Some(font_family),
                line_height: Some(line_height),
                text_align: Some("left".to_owned()),
                vertical_align: Some("top".to_owned()),
                ..base.clone()
            }
        };
        for i in 0..=self.columns {
            let n = i64::from(i) - i64::from(self.origin_column);
            if n != 0 {
                let at = Point::new(b.x0 + f64::from(i) * u, o.y + LABEL_GAP);
                out.push(label(&n.to_string(), LABEL_SIZE, at, Vec2::new(0.5, 0.0)));
            }
        }
        for j in 0..=self.rows {
            let n = i64::from(self.origin_row) - i64::from(j);
            if n != 0 {
                let at = Point::new(o.x - LABEL_GAP * 1.5, b.y0 + f64::from(j) * u);
                out.push(label(&n.to_string(), LABEL_SIZE, at, Vec2::new(1.0, 0.5)));
            }
        }
        out.push(label("0", LABEL_SIZE, o + Vec2::new(-LABEL_GAP * 1.5, LABEL_GAP), Vec2::new(1.0, 0.0)));
        let name_size = LABEL_SIZE * 1.25;
        out.push(label("x", name_size, Point::new(b.x1 + over, o.y - LABEL_GAP * 2.0), Vec2::new(0.5, 1.0)));
        out.push(label("y", name_size, Point::new(o.x + LABEL_GAP * 3.0, b.y0 - over), Vec2::new(0.0, 0.5)));
        out
    }
}

/// The intersection of any plane in `elements` near `p`.
pub fn snap<'a>(elements: impl IntoIterator<Item = &'a Element>, p: Point) -> Option<Point> {
    elements.into_iter().filter_map(Plane::of).find_map(|plane| plane.snap(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(els: &[Element]) -> Vec<&str> {
        els.iter().filter_map(|e| e.text.as_deref()).collect()
    }

    #[test]
    fn a_drag_makes_an_even_plane_centered_on_the_origin() {
        let plane = Plane::from_drag(Point::new(100.0, 100.0), Point::new(100.0 + 7.4 * UNIT, 100.0 + 3.0 * UNIT), false);
        assert_eq!((plane.columns, plane.rows), (8, 4));
        assert_eq!(plane.origin(), Point::new(100.0 + 4.0 * UNIT, 100.0 + 2.0 * UNIT));

        let els = plane.elements("#1e1e1e", 5);
        let labels = texts(&els);
        for n in ["-4", "-1", "1", "4", "-2", "2", "0", "x", "y"] {
            assert!(labels.contains(&n), "{n} missing from {labels:?}");
        }
        // Grid, two axes, 8 x numbers, 4 y numbers, 0, x, y.
        assert_eq!(els.len(), 3 + 8 + 4 + 3);
        assert_eq!(els.iter().filter(|e| e.kind == Kind::Arrow && e.start_arrowhead.is_some()).count(), 2);
    }

    #[test]
    fn dragging_backwards_or_first_quadrant_only() {
        let plane = Plane::from_drag(Point::new(400.0, 400.0), Point::new(400.0 - 3.0 * UNIT, 400.0 - 2.0 * UNIT), true);
        assert_eq!((plane.columns, plane.rows, plane.origin_column, plane.origin_row), (3, 2, 0, 2));
        assert_eq!(plane.top_left, Point::new(400.0 - 3.0 * UNIT, 400.0 - 2.0 * UNIT));
        assert_eq!(plane.origin(), Point::new(400.0 - 3.0 * UNIT, 400.0));
        let els = plane.elements("#1e1e1e", 5);
        assert!(!texts(&els).contains(&"-1"));
        assert!(els.iter().filter(|e| e.kind == Kind::Arrow).all(|e| e.start_arrowhead.is_none()));
    }

    #[test]
    fn the_grid_line_covers_every_row_and_column() {
        for (columns, rows) in [(2, 2), (3, 2), (4, 5), (1, 1)] {
            let plane = Plane { top_left: Point::ZERO, unit: 10.0, columns, rows, origin_column: 0, origin_row: rows };
            let grid = &plane.elements("#000", 5)[0];
            let segs: Vec<_> = grid.points.windows(2).map(|w| (w[0], w[1])).collect();
            for i in 0..=columns {
                let x = f64::from(i) * 10.0;
                let full = segs.iter().any(|(a, b)| a[0] == x && b[0] == x && (a[1] - b[1]).abs() == f64::from(rows) * 10.0);
                assert!(full, "column {i} of {columns}×{rows}");
            }
            for j in 0..=rows {
                let y = f64::from(j) * 10.0;
                let full = segs.iter().any(|(a, b)| a[1] == y && b[1] == y && (a[0] - b[0]).abs() == f64::from(columns) * 10.0);
                assert!(full, "row {j} of {columns}×{rows}");
            }
        }
    }

    #[test]
    fn plane_survives_moves_and_resizes_and_snaps_points() {
        let plane = Plane::from_drag(Point::ZERO, Point::new(4.0 * UNIT, 4.0 * UNIT), false);
        let mut grid = plane.elements("#000", 5).remove(0);
        assert_eq!(Plane::of(&grid), Some(plane));
        // Moved and scaled to half the size.
        grid.x += 15.0;
        for p in &mut grid.points {
            p[0] /= 2.0;
            p[1] /= 2.0;
        }
        (grid.width, grid.height) = (grid.width / 2.0, grid.height / 2.0);
        let moved = Plane::of(&grid).unwrap();
        assert_eq!(moved.unit, UNIT / 2.0);
        assert_eq!(moved.origin(), Point::new(15.0 + UNIT, UNIT));
        let near = moved.origin() + Vec2::new(UNIT / 2.0 + 2.0, -3.0);
        assert_eq!(snap([&grid], near), Some(moved.origin() + Vec2::new(UNIT / 2.0, 0.0)));
        // Between intersections, or off the plane: left alone.
        assert_eq!(snap([&grid], moved.origin() + Vec2::new(UNIT / 4.0, UNIT / 4.0)), None);
        assert_eq!(snap([&grid], Point::new(500.0, 500.0)), None);
    }
}
