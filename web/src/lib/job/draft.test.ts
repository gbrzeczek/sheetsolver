import { describe, expect, it } from "vitest";
import { emptyPanel, emptyPart, namesFor, newDraft, parseLength, toRequest, type Draft } from "./draft";
import { place } from "./problems";

function draft(): Draft {
  const d = newDraft();
  d.parts = [
    { ...emptyPart(), id: "Side", length: "720", width: "560", quantity: "2", grain: true },
    { ...emptyPart(), length: "400", width: "300,5" },
    emptyPart(),
  ];
  d.panels = [{ ...emptyPanel(), length: "2440", width: "1220" }];
  return d;
}

describe("a new job", () => {
  it("starts with the FIELDS.md defaults", () => {
    const d = newDraft();
    expect([d.objective, d.kerf, d.stages, d.cutType, d.firstCuts, d.timeLimit]).toEqual([
      "bin_packing", "3.2", 3, "non_exact", "either", "10",
    ]);
  });
});

describe("generated names", () => {
  it("fill empty names by row number, skipping names in use", () => {
    expect(namesFor([{ id: "Side" }, { id: "" }, { id: "" }], "Part")).toEqual(["Side", "Part 2", "Part 3"]);
    expect(namesFor([{ id: "" }, { id: "Part 1" }], "Part")).toEqual(["Part 2", "Part 1"]);
    expect(namesFor([{ id: "" }], "Panel")).toEqual(["Panel 1"]);
  });
});

describe("the request", () => {
  it("sends only typed sizes, a generated name for an empty one, and no trailing empty row", () => {
    const { request, problems } = toRequest(draft());
    expect(problems).toEqual([]);
    expect(request.instance.parts).toEqual([
      { id: "Side", length: 720, width: 560, quantity: 2, grain: true },
      { id: "Part 2", length: 400, width: 300.5, quantity: 1, grain: false },
    ]);
    expect(request.instance.panels).toEqual([{ id: "Panel 1", length: 2440, width: 1220 }]);
    expect(request.instance.saw).toEqual({ kerf: 3.2, stages: 3, cut_type: "non_exact", first_cuts: "either" });
    expect(request.time_limit).toBe(10);
  });

  it("reports problems with the API's paths and wording", () => {
    const d = draft();
    d.parts[1]!.width = "320.55";
    d.parts[0]!.quantity = "0";
    d.kerf = "-1";
    d.timeLimit = "900";
    const { problems } = toRequest(d);
    expect(problems).toContainEqual({ field: "instance.parts[1].width", message: "length 320.55 mm has more than one decimal; lengths are in steps of 0.1 mm" });
    expect(problems).toContainEqual({ field: "instance.parts[0].quantity", message: "quantity must be at least 1" });
    expect(problems.map((p) => p.field)).toContain("instance.saw.kerf");
    expect(problems.map((p) => p.field)).toContain("time_limit");
  });

  it("keeps positions of rows in the middle, even empty ones", () => {
    const d = draft();
    d.parts.splice(1, 0, emptyPart());
    const { problems } = toRequest(d);
    expect(problems.map((p) => p.field)).toEqual(["instance.parts[1].length", "instance.parts[1].width"]);
  });

  it("finds duplicate names, typed or generated", () => {
    const d = draft();
    d.parts[0]!.id = "Part 2";
    d.parts[1]!.id = "Part 2";
    expect(toRequest(d).problems).toContainEqual({ field: "instance.parts[1].id", message: 'part name "Part 2" is used more than once' });
  });
});

describe("lengths", () => {
  it("accept a comma and trailing zeros, reject a second decimal", () => {
    expect(parseLength("3,2", "kerf", 0)).toEqual({ ok: true, value: 3.2 });
    expect(parseLength("18.50", "width", 0.1)).toEqual({ ok: true, value: 18.5 });
    expect(parseLength("3.25", "width", 0.1).ok).toBe(false);
    expect(parseLength("0", "width", 0.1).ok).toBe(false);
    expect(parseLength("abc", "width", 0.1).ok).toBe(false);
  });
});

describe("problem places", () => {
  it("map API paths to rows, settings and lists", () => {
    expect(place("instance.parts[1].width")).toEqual({ section: "parts", row: 1, field: "Part.width", label: "Row 2, Width" });
    expect(place("instance.saw.kerf")).toMatchObject({ section: "settings", field: "Saw.kerf" });
    expect(place("instance.panels")).toMatchObject({ section: "panels", label: "Panels" });
    expect(place(null)).toMatchObject({ section: "page" });
  });
});
