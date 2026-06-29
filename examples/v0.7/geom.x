module geom

pub struct Point {
    i32 x;
    i32 y;
}

pub Point origin() {
    Point p = { 5, 6 };
    return p;
}

pub i32 sum(Point p) {
    return p.x + p.y;
}
