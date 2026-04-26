import { getPanelForApp, hasPanel } from "@/components/panels/panelRegistry";
import React from "react";
import { render } from "@testing-library/react";

describe("panelRegistry — World Monitor integration", () => {
  test("registers world-monitor in PANEL_MAP", () => {
    expect(hasPanel("world-monitor")).toBe(true);
  });

  test("getPanelForApp returns a React node for world-monitor", () => {
    const node = getPanelForApp("world-monitor");
    // render should not throw — component may lazy-load an iframe
    expect(() => render(<>{node}</>)).not.toThrow();
  });
});
