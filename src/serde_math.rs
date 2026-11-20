// serde helpers for macroquad math types.

pub mod v2 {
    use macroquad::prelude::Vec2;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Repr {
        x: f32,
        y: f32,
    }

    pub fn serialize<S: Serializer>(v: &Vec2, s: S) -> Result<S::Ok, S::Error> {
        Repr { x: v.x, y: v.y }.serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec2, D::Error> {
        let r = Repr::deserialize(d)?;
        Ok(Vec2::new(r.x, r.y))
    }
}

pub mod v2vec {
    use macroquad::prelude::Vec2;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Repr {
        x: f32,
        y: f32,
    }

    pub fn serialize<S: Serializer>(v: &[Vec2], s: S) -> Result<S::Ok, S::Error> {
        let r: Vec<Repr> = v.iter().map(|q| Repr { x: q.x, y: q.y }).collect();
        r.serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec2>, D::Error> {
        let r = Vec::<Repr>::deserialize(d)?;
        Ok(r.into_iter().map(|q| Vec2::new(q.x, q.y)).collect())
    }
}

pub mod rect {
    use macroquad::prelude::Rect;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Repr {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    pub fn serialize<S: Serializer>(v: &Rect, s: S) -> Result<S::Ok, S::Error> {
        Repr { x: v.x, y: v.y, w: v.w, h: v.h }.serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Rect, D::Error> {
        let r = Repr::deserialize(d)?;
        Ok(Rect::new(r.x, r.y, r.w, r.h))
    }
}
