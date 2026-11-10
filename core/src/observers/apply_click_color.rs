use crate::components::{ClickColor, Sprite};
use crate::{SimEvent, SimEvents, SimId};
use bevy_ecs::prelude::{Entity, EntityEvent, On, Query, ResMut};

#[derive(EntityEvent)]
pub(crate) struct Clicked {
    pub(crate) entity: Entity,
    pub(crate) id: SimId,
    pub(crate) tick: u64,
}

pub(crate) fn apply_click_color(
    click: On<Clicked>,
    mut sprites: Query<(&mut Sprite, &ClickColor)>,
    mut events: ResMut<SimEvents>,
) {
    if let Ok((mut sprite, color)) = sprites.get_mut(click.entity)
        && sprite.color != color.0
    {
        sprite.color.clone_from(&color.0);
        events.0.push(SimEvent {
            tick: click.tick,
            id: click.id,
            color: color.0.clone(),
        });
    }
}
