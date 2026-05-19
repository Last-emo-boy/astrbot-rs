import type { Component, JSX } from "solid-js";

interface CardProps {
  title?: string;
  actions?: JSX.Element;
  children: JSX.Element;
}

export const Card: Component<CardProps> = (props) => (
  <div class="card">
    {(props.title || props.actions) && (
      <div class="toolbar">
        {props.title && <h3 class="card__title" style={{ margin: 0 }}>{props.title}</h3>}
        <div class="toolbar__spacer" />
        {props.actions}
      </div>
    )}
    <div>{props.children}</div>
  </div>
);

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  actions?: JSX.Element;
}

export const PageHeader: Component<PageHeaderProps> = (props) => (
  <div class="page-header">
    <div>
      <h1 class="page-header__title">{props.title}</h1>
      {props.subtitle && <div class="page-header__subtitle">{props.subtitle}</div>}
    </div>
    {props.actions && <div class="row">{props.actions}</div>}
  </div>
);

export const EmptyState: Component<{ message?: string }> = (props) => (
  <div class="empty-state">{props.message ?? "暂无数据"}</div>
);

export const Loading: Component = () => <div class="empty-state">加载中…</div>;
