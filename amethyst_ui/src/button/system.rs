use std::collections::HashMap;
use derivative::Derivative;

use amethyst_core::{
    ecs::{Entity, ReadExpect, Resources, System, SystemData, Write, WriteStorage},
    shrev::{EventChannel, ReaderId},
    ParentHierarchy,
};
use amethyst_assets::Handle;
use crate::{UiButtonAction, UiButtonActionType::*, UiText, render::UiRenderer};

struct ActionChangeStack<T: Clone + PartialEq> {
    initial_value: T,
    stack: Vec<T>,
}

impl<T> ActionChangeStack<T>
where
    T: Clone + PartialEq,
{
    pub fn new(initial_value: T) -> Self {
        ActionChangeStack {
            initial_value,
            stack: Vec::new(),
        }
    }

    pub fn add(&mut self, change: T) {
        self.stack.push(change);
    }

    pub fn remove(&mut self, change: &T) -> Option<T> {
        if let Some(idx) = self.stack.iter().position(|it| it == change) {
            Some(self.stack.remove(idx))
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn current(&self) -> T {
        if self.stack.is_empty() {
            self.initial_value.clone()
        } else {
            self.stack
                .iter()
                .last()
                .map(T::clone)
                .expect("Unreachable: Just checked that stack is not empty")
        }
    }
}

/// This system manages button mouse events.  It changes images and text colors, as well as playing audio
/// when necessary.
///
/// It's automatically registered with the `UiBundle`.
#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct UiButtonSystem<R: UiRenderer> {
    event_reader: Option<ReaderId<UiButtonAction<R>>>,
    set_textures: HashMap<Entity, ActionChangeStack<Handle<R::Texture>>>,
    set_text_colors: HashMap<Entity, ActionChangeStack<[f32; 4]>>,
}

impl<R> UiButtonSystem<R> where R: UiRenderer {
    /// Creates a new instance of this structure
    pub fn new() -> Self {
        Default::default()
    }
}

impl<'s, R> System<'s> for UiButtonSystem<R> where R: UiRenderer {
    type SystemData = (
        WriteStorage<'s, Handle<R::Texture>>,
        WriteStorage<'s, UiText>,
        ReadExpect<'s, ParentHierarchy>,
        Write<'s, EventChannel<UiButtonAction<R>>>,
    );

    fn setup(&mut self, res: &mut Resources) {
        Self::SystemData::setup(res);
        self.event_reader = Some(
            res.fetch_mut::<EventChannel<UiButtonAction<R>>>()
                .register_reader(),
        );
    }

    fn run(
        &mut self,
        (mut image_storage, mut text_storage, hierarchy, button_events): Self::SystemData,
    ) {
        let event_reader = self
            .event_reader
            .as_mut()
            .expect("`UiButtonSystem::setup` was not called before `UiButtonSystem::run`");

        for event in button_events.read(event_reader) {
            match event.event_type {
                SetTextColor(ref color) => {
                    for &child in hierarchy.children(event.target) {
                        if let Some(text) = text_storage.get_mut(child) {
                            // found the text. push its original color if
                            // it's not there yet
                            self.set_text_colors
                                .entry(event.target)
                                .or_insert_with(|| ActionChangeStack::new(text.color))
                                .add(*color);

                            text.color = *color;
                        }
                    }
                }
                UnsetTextColor(ref color) => {
                    for &child in hierarchy.children(event.target) {
                        if let Some(text) = text_storage.get_mut(child) {
                            // first, remove the color we were told to unset
                            if !self.set_text_colors.contains_key(&event.target) {
                                // nothing to do!
                                continue;
                            }

                            self.set_text_colors
                                .get_mut(&event.target)
                                .and_then(|it| it.remove(color));

                            text.color = self.set_text_colors[&event.target].current();

                            if self.set_text_colors[&event.target].is_empty() {
                                self.set_text_colors.remove(&event.target);
                            }
                        }
                    }
                }
                SetTexture(ref texture_handle) => {
                    if let Some(image) = image_storage.get_mut(event.target) {
                        self.set_textures
                            .entry(event.target)
                            .or_insert_with(|| ActionChangeStack::new(image.clone()))
                            .add(texture_handle.clone());

                        *image = texture_handle.clone();
                    }
                }
                UnsetTexture(ref texture_handle) => {
                    if let Some(image) = image_storage.get_mut(event.target) {
                        if !self.set_textures.contains_key(&event.target) {
                            continue;
                        }

                        self.set_textures
                            .get_mut(&event.target)
                            .and_then(|it| it.remove(texture_handle));

                        *image = self.set_textures[&event.target].current();

                        if self.set_textures[&event.target].is_empty() {
                            self.set_textures.remove(&event.target);
                        }
                    }
                }
            };
        }
    }
}
