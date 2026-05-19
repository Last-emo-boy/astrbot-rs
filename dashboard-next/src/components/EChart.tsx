import { createEffect, onCleanup, onMount, type Component } from "solid-js";
import * as echarts from "echarts";

interface EChartProps {
  option: echarts.EChartsOption;
  height?: string;
  class?: string | undefined;
}

/**
 * Thin SolidJS wrapper around ECharts. Owns a `<div>` element, lazily
 * instantiates the chart on mount, and re-applies the option whenever it
 * changes. Resize is handled via a `ResizeObserver` on the container.
 */
export const EChart: Component<EChartProps> = (props) => {
  let container: HTMLDivElement | undefined;
  let chart: echarts.ECharts | undefined;
  let resizeObserver: ResizeObserver | undefined;

  onMount(() => {
    if (!container) return;
    chart = echarts.init(container);
    chart.setOption(props.option);
    resizeObserver = new ResizeObserver(() => {
      chart?.resize();
    });
    resizeObserver.observe(container);
  });

  createEffect(() => {
    if (chart) {
      chart.setOption(props.option, { notMerge: false });
    }
  });

  onCleanup(() => {
    resizeObserver?.disconnect();
    chart?.dispose();
  });

  return (
    <div
      ref={(el) => (container = el)}
      class={["echart-host", props.class].filter(Boolean).join(" ")}
      style={{ width: "100%", height: props.height ?? "320px" }}
    />
  );
};

export default EChart;
