use std::{rc::Rc, sync::Arc};

use floem::{
    IntoView, View,
    reactive::{ReadSignal, SignalGet, SignalUpdate},
    style::CursorStyle,
    views::{
        Decorators, container, dyn_container, dyn_stack, label, scroll,
        stack_from_iter,
    },
};
use phidi_rpc::agent::{AgentCapabilityResponse, CapabilityResponse};

use crate::{
    agent_workbench::{
        AgentCapabilityKind, AgentRunState, request_detail_lines,
        response_detail_lines,
    },
    config::{PhidiConfig, color::PhidiColor},
    panel::{
        data::PanelSection,
        position::PanelPosition,
        view::{PanelBuilder, panel_header},
    },
    window_tab::WindowTabData,
};

pub fn agent_panel(
    window_tab_data: Rc<WindowTabData>,
    position: PanelPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let is_bottom = position.is_bottom();
    PanelBuilder::new(config, position)
        .add_style(
            "Request",
            request_section(window_tab_data.clone(), config),
            window_tab_data
                .panel
                .section_open(PanelSection::AtlasRequest),
            move |s| {
                s.border_color(config.get().color(PhidiColor::PHIDI_BORDER))
                    .apply_if(is_bottom, |s| s.border_right(1.0))
                    .apply_if(!is_bottom, |s| s.border_bottom(1.0))
            },
        )
        .add(
            "Response",
            response_section(window_tab_data.clone(), config),
            window_tab_data
                .panel
                .section_open(PanelSection::AtlasResponse),
        )
        .add(
            "Recent Runs",
            history_section(window_tab_data.clone(), config),
            window_tab_data
                .panel
                .section_open(PanelSection::AtlasHistory),
        )
        .build()
        .debug_name("Atlas Panel")
}

fn request_section(
    window_tab_data: Rc<WindowTabData>,
    config: ReadSignal<Arc<PhidiConfig>>,
) -> impl View {
    let button_row = stack_from_iter(AgentCapabilityKind::ALL.into_iter().map({
        let window_tab_data = window_tab_data.clone();
        move |capability| {
            capability_button(window_tab_data.clone(), capability, config).into_any()
        }
    }))
    .style(|s| {
        s.flex_row()
            .flex_wrap(floem::taffy::FlexWrap::Wrap)
            .gap(8.0)
            .padding_horiz(10.0)
    });

    let run = window_tab_data.agent_workbench.active_run;
    stack_from_iter([
        button_row.into_any(),
        dyn_container(
            move || run.get(),
            move |run| match run {
                Some(run) => detail_lines(
                    request_detail_lines(&run.request),
                    config,
                    PhidiColor::PANEL_FOREGROUND,
                )
                .into_any(),
                None => label(|| {
                    "Run any Atlas capability from the command palette or the buttons above."
                        .to_string()
                })
                .style(move |s| {
                    s.padding_horiz(10.0)
                        .padding_bottom(8.0)
                        .color(config.get().color(PhidiColor::EDITOR_DIM))
                })
                .into_any(),
            },
        )
        .into_any(),
    ])
    .style(|s| s.flex_col().row_gap(8.0).padding_top(8.0))
}

fn response_section(
    window_tab_data: Rc<WindowTabData>,
    config: ReadSignal<Arc<PhidiConfig>>,
) -> impl View {
    let run = window_tab_data.agent_workbench.active_run;
    dyn_container(
        move || run.get(),
        move |run| match run {
            Some(run) => {
                let mut views = vec![
                    panel_header(
                        format!(
                            "{} · {}",
                            run.summary.title, run.summary.status_label
                        ),
                        config,
                    )
                    .into_any(),
                    label(move || run.summary.detail.clone())
                        .style(move |s| {
                            s.padding_horiz(10.0).padding_vert(8.0).line_height(1.6)
                        })
                        .into_any(),
                ];

                match &run.state {
                    AgentRunState::Running => {
                        views.push(
                            label(|| {
                                "Waiting on the background worker...".to_string()
                            })
                            .style(move |s| {
                                s.padding_horiz(10.0).padding_bottom(8.0).color(
                                    config.get().color(PhidiColor::EDITOR_DIM),
                                )
                            })
                            .into_any(),
                        );
                    }
                    AgentRunState::Complete { response } => {
                        views.push(
                            detail_lines(
                                response_detail_lines(response),
                                config,
                                status_color(response),
                            )
                            .into_any(),
                        );
                    }
                }

                scroll(stack_from_iter(views).style(|s| s.flex_col()))
                    .style(|s| s.size_pct(100.0, 100.0))
                    .into_any()
            }
            None => label(|| "No Atlas run selected yet.".to_string())
                .style(move |s| {
                    s.padding(10.0)
                        .color(config.get().color(PhidiColor::EDITOR_DIM))
                })
                .into_any(),
        },
    )
}

fn history_section(
    window_tab_data: Rc<WindowTabData>,
    config: ReadSignal<Arc<PhidiConfig>>,
) -> impl View {
    let runs = window_tab_data.agent_workbench.recent_runs;
    let active_run = window_tab_data.agent_workbench.active_run;
    scroll(
        dyn_stack(
            move || runs.get(),
            |run| run.id,
            move |run| {
                let title = run.summary.title.clone();
                let status = run.summary.status_label.clone();
                let detail = run.summary.detail.clone();
                let selected_run = run.clone();
                container(
                    stack_from_iter([
                        label(move || title.clone())
                            .style(|s| s.font_bold().min_width(0.0))
                            .into_any(),
                        label(move || status.clone())
                            .style(move |s| {
                                s.color(config.get().color(PhidiColor::EDITOR_DIM))
                            })
                            .into_any(),
                        label(move || detail.clone())
                            .style(|s| s.min_width(0.0).line_height(1.5))
                            .into_any(),
                    ])
                    .style(|s| s.flex_col().row_gap(4.0).width_pct(100.0)),
                )
                .on_click_stop(move |_| {
                    active_run.set(Some(selected_run.clone()));
                })
                .style(move |s| {
                    s.width_pct(100.0).padding(10.0).hover(|s| {
                        s.cursor(CursorStyle::Pointer).background(
                            config.get().color(PhidiColor::PANEL_HOVERED_BACKGROUND),
                        )
                    })
                })
            },
        )
        .style(|s| s.flex_col().width_pct(100.0)),
    )
    .style(|s| s.absolute().size_pct(100.0, 100.0))
}

fn capability_button(
    window_tab_data: Rc<WindowTabData>,
    capability: AgentCapabilityKind,
    config: ReadSignal<Arc<PhidiConfig>>,
) -> impl View {
    label(move || capability.title().to_string())
        .on_click_stop(move |_| {
            window_tab_data.run_agent_capability(capability);
        })
        .style(move |s| {
            s.padding_horiz(10.0)
                .padding_vert(6.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(config.get().color(PhidiColor::PHIDI_BORDER))
                .hover(|s| {
                    s.cursor(CursorStyle::Pointer).background(
                        config.get().color(PhidiColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
        })
}

fn detail_lines(
    lines: Vec<String>,
    config: ReadSignal<Arc<PhidiConfig>>,
    color: &'static str,
) -> impl View {
    let color = color.to_string();
    stack_from_iter(lines.into_iter().map(move |line| {
        let color = color.clone();
        label(move || line.clone())
            .style(move |s| {
                s.padding_horiz(10.0)
                    .padding_bottom(6.0)
                    .line_height(1.6)
                    .color(config.get().color(color.as_str()))
            })
            .into_any()
    }))
    .style(|s| s.flex_col().padding_bottom(6.0))
}

fn status_color(response: &AgentCapabilityResponse) -> &'static str {
    match response {
        AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Error {
            ..
        })
        | AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Error {
            ..
        })
        | AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Error { .. },
        )
        | AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Error {
            ..
        })
        | AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Error {
            ..
        })
        | AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Error {
            ..
        }) => PhidiColor::PHIDI_ERROR,
        AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Timeout {
            ..
        })
        | AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Timeout {
            ..
        })
        | AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Timeout { .. },
        )
        | AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Timeout {
            ..
        })
        | AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Timeout {
            ..
        })
        | AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Timeout {
            ..
        }) => PhidiColor::PHIDI_WARN,
        _ => PhidiColor::PANEL_FOREGROUND,
    }
}
