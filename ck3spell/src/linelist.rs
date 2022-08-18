// Much of the code for this LineList widget is based on druid::widget::List,
// which is under the Apache License, Version 2.0

use druid::widget::prelude::*;
use druid::widget::ListIter;
use druid::{Command, Point, Rect, Target, WidgetPod};
use std::cmp::Ordering;

use crate::commands::{QUERY_LINE_LAYOUT_REGION, REPLY_LINE_LAYOUT_REGION};
use crate::LineInfo;

pub struct LineList {
    closure: Box<dyn Fn() -> Box<dyn Widget<LineInfo>>>,
    children: Vec<WidgetPod<LineInfo, Box<dyn Widget<LineInfo>>>>,
    old_bc: BoxConstraints,
}

impl LineList {
    pub fn new<W: Widget<LineInfo> + 'static>(closure: impl Fn() -> W + 'static) -> Self {
        LineList {
            closure: Box::new(move || Box::new(closure())),
            children: Vec::new(),
            old_bc: BoxConstraints::tight(Size::ZERO),
        }
    }

    fn update_child_count(&mut self, data: &impl ListIter<LineInfo>, _env: &Env) -> bool {
        let len = self.children.len();
        match len.cmp(&data.data_len()) {
            Ordering::Greater => self.children.truncate(data.data_len()),
            Ordering::Less => data.for_each(|_, i| {
                if i >= len {
                    let child = WidgetPod::new((self.closure)());
                    self.children.push(child);
                }
            }),
            Ordering::Equal => (),
        }
        len != data.data_len()
    }

    fn child_layout_rect(&self, nr: usize) -> Option<Rect> {
        Some(self.children.get(nr)?.layout_rect())
    }
}

impl<T: ListIter<LineInfo>> Widget<T> for LineList {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let mut children = self.children.iter_mut();
        data.for_each_mut(|child_data, _| {
            if let Some(child) = children.next() {
                child.event(ctx, event, child_data, env);
            }
        });

        if let Event::Command(command) = event {
            if let Some(&linenr) = command.get(QUERY_LINE_LAYOUT_REGION) {
                if let Some(region) = self.child_layout_rect(linenr - 1) {
                    let command = Command::new(REPLY_LINE_LAYOUT_REGION, region, Target::Auto);
                    ctx.submit_notification(command);
                }
            }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            if self.update_child_count(data, env) {
                ctx.children_changed();
            }
        }

        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.lifecycle(ctx, event, child_data, env);
            }
        });
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.update(ctx, child_data, env);
            }
        });

        if self.update_child_count(data, env) {
            ctx.children_changed();
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("LineList");
        let mut pos: f64 = 0.0;
        let mut width: f64 = 0.0;
        let mut paint_rect = Rect::ZERO;

        let bc_changed = self.old_bc != *bc;
        self.old_bc = *bc;

        let mut children = self.children.iter_mut();
        let child_bc = BoxConstraints::new(
            Size::new(bc.min().width, 0.0),
            Size::new(bc.max().width, f64::INFINITY),
        );
        data.for_each(|child_data, _| {
            let child = match children.next() {
                Some(child) => child,
                None => {
                    return;
                }
            };

            let child_size = if bc_changed || child.layout_requested() {
                child.layout(ctx, &child_bc, child_data, env)
            } else {
                child.layout_rect().size()
            };

            let child_pos = Point::new(0.0, pos);
            child.set_origin(ctx, child_data, env, child_pos);
            paint_rect = paint_rect.union(child.paint_rect());
            width = width.max(child_size.width);
            pos += child_size.height;
        });

        bc.constrain(Size::new(width, pos))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.paint(ctx, child_data, env);
            }
        });
    }
}
