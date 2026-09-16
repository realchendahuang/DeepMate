import { describe, expect, it } from "vitest";
import { inferSurface, scenarioInitial, scenarioLayers } from "./scenario";
import type { Profile } from "@/shared/api/api";

const profile = (id: string, description: string | null): Profile => ({
  id,
  name: id,
  description,
});

describe("scenarioLayers", () => {
  it("returns empty when the description is missing", () => {
    expect(scenarioLayers(null)).toEqual({ bundles: [], blurb: null });
    expect(scenarioLayers("")).toEqual({ bundles: [], blurb: null });
  });

  it("splits a bundles fallback into chips", () => {
    expect(scenarioLayers("bundles: @deepseek-ai/dsh-base, @deepseek-ai/dsh-web-app")).toEqual({
      bundles: ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"],
      blurb: null,
    });
  });

  it("treats any other string as a real blurb", () => {
    expect(scenarioLayers("日常打字")).toEqual({ bundles: [], blurb: "日常打字" });
  });
});

describe("scenarioInitial", () => {
  it("uppercases the first grapheme", () => {
    expect(scenarioInitial("web")).toBe("W");
    expect(scenarioInitial("公开")).toBe("公");
    expect(scenarioInitial("")).toBe("?");
  });
});

describe("inferSurface", () => {
  it("keeps a resolved surface as-is", () => {
    expect(inferSurface(profile("dev", null), "task")).toBe("task");
    expect(inferSurface(profile("dev", null), "web")).toBe("web");
  });

  it("treats the default scenario as a web scenario", () => {
    expect(inferSurface(profile("web", null), "undetermined")).toBe("web");
  });

  it("derives task scenarios from headless bundles", () => {
    expect(
      inferSurface(
        profile("cli", "bundles: @deepseek-ai/dsh-base, @deepseek-ai/dsh-headless"),
        "undetermined",
      ),
    ).toBe("task");
  });

  it("derives web scenarios from web-app or web-ui bundles", () => {
    expect(inferSurface(profile("app", "bundles: @deepseek-ai/dsh-web-app"), "undetermined")).toBe(
      "web",
    );
    expect(inferSurface(profile("ui", "bundles: @deepseek-ai/dsh-web-ui"), "undetermined")).toBe(
      "web",
    );
  });

  it("stays undetermined without any signal", () => {
    expect(inferSurface(profile("dev", "bundles: @deepseek-ai/dsh-base"), "undetermined")).toBe(
      "undetermined",
    );
  });
});
