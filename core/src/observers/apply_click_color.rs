use crate::components::{ClickColor, Sprite};
use bevy_ecs::prelude::{Entity, EntityEvent, On, Query};

#[derive(EntityEvent)]
pub(crate) struct Clicked {
    pub(crate) entity: Entity,
}

pub(crate) fn apply_click_color(
    click: On<Clicked>,
    mut sprites: Query<(&mut Sprite, &ClickColor)>,
) {
    if let Ok((mut sprite, color)) = sprites.get_mut(click.entity) {
        sprite.color.clone_from(&color.0);
    }
}
