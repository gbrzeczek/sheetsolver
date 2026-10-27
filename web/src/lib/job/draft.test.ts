import { describe, expect, it } from "vitest";
import { adoptKeys, draftMatches, emptyPanel, emptyPart, fromInstance, namesFor, newDraft, parseLength, toRequest, type Draft } from "./draft";
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
      "bin_packing", "0", 3, "non_exact", "either", "10",
    ]);
  });
});

describe("row keys", () => {
  it("are never reused after a stored draft is taken in", () => {
    // A draft stored on an earlier visit, numbered from 1 then: its keys are
    // the ones this visit would hand out next.
    const stored = newDraft();
    const [part, panel] = [emptyPart(), emptyPanel()];
    const next = panel.key + 1;
    stored.parts = [{ ...part, key: next }, { ...part, key: next + 2 }];
    stored.panels = [{ ...panel, key: next + 1 }];
    adoptKeys(stored);
    const keys = [...stored.parts, ...stored.panels].map((r) => r.key);
    expect(keys).not.toContain(emptyPart().key);
    expect(keys).not.toContain(emptyPanel().key);
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
    expect(request.instance.saw).toEqual({ kerf: 0, stages: 3, cut_type: "non_exact", first_cuts: "either" });
  });

  it("sends an empty kerf as 0 mm", () => {
    const d = draft();
    d.kerf = " ";
    const { request, problems } = toRequest(d);
    expect(problems).toEqual([]);
    expect(request.instance.saw.kerf).toBe(0);
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
    expect(parseLength("1 040.5", "length", 0.1)).toEqual({ ok: true, value: 1040.5 });
    expect(parseLength("2\u202f440", "length", 0.1)).toEqual({ ok: true, value: 2440 });
    expect(parseLength("1 04", "length", 0.1).ok).toBe(false);
  });

  it("reject lengths beyond the API's maximum", () => {
    expect(parseLength("100000", "length", 0.1)).toEqual({ ok: true, value: 100000 });
    expect(parseLength("100000.1", "length", 0.1)).toEqual({ ok: false, message: "length must be at most 100000 mm" });
    expect(parseLength("1000000000000", "saw blade thickness", 0).ok).toBe(false);
  });

  it("reject three digits after a comma or point, which may be a thousands separator", () => {
    expect(parseLength("1,200", "length", 0.1)).toEqual({ ok: false, message: "write 1,200 without a separator: 1200 for 1200 mm, or 1.2 for 1.2 mm" });
    expect(parseLength("2.500", "width", 0.1).ok).toBe(false);
    expect(parseLength("2,440", "length", 0.1).ok).toBe(false);
    expect(parseLength("1200,5", "length", 0.1)).toEqual({ ok: true, value: 1200.5 });
    expect(parseLength("18.5000", "width", 0.1)).toEqual({ ok: true, value: 18.5 });
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

describe("the job name", () => {
  it("is sent trimmed, left out when blank, and checked like the API does", () => {
    const d = draft();
    d.name = "  Kitchen units ";
    expect(toRequest(d).request.name).toBe("Kitchen units");
    d.name = "   ";
    expect(toRequest(d).request).not.toHaveProperty("name");
    d.name = "x".repeat(101);
    expect(toRequest(d).problems).toContainEqual({ field: "name", message: "job name must be at most 100 characters, not 101" });
    d.name = "two\nlines";
    expect(toRequest(d).problems).toContainEqual({ field: "name", message: "job name must be one line, without tabs or line breaks" });
  });
});

describe("a plan for an older job", () => {
  const sent = () => toRequest(draft()).request.instance;

  it("matches the draft it was sent from, and the server's echo with defaults filled in", () => {
    expect(draftMatches(draft(), sent())).toBe(true);
    const echo = sent();
    echo.panels = echo.panels.map((p) => ({ ...p, quantity: null, cost: null }));
    echo.parts = echo.parts.map((p) => ({ ...p, value: null }));
    expect(draftMatches(draft(), echo)).toBe(true);
  });

  it("matches again after Edit job, whose generated names come back empty", () => {
    expect(draftMatches(fromInstance(sent(), 10), sent())).toBe(true);
  });

  it("is out of date when a size, a quantity, the grain or a setting changes", () => {
    const changes: ((d: Draft) => void)[] = [
      (d) => (d.parts[0]!.width = "561"),
      (d) => (d.parts[1]!.quantity = "2"),
      (d) => (d.parts[1]!.grain = true),
      (d) => (d.panels[0]!.quantity = "3"),
      (d) => (d.kerf = "4"),
      (d) => (d.stages = 2),
      (d) => (d.parts[1]!.id = "Back"),
    ];
    for (const change of changes) {
      const d = draft();
      change(d);
      expect(draftMatches(d, sent())).toBe(false);
    }
  });

  it("is not out of date for the job name or the time limit, which do not change the plan", () => {
    const d = draft();
    d.name = "Kitchen units";
    d.timeLimit = "60";
    expect(draftMatches(d, sent())).toBe(true);
  });

  it("is out of date while the draft cannot be sent", () => {
    const d = draft();
    d.parts[0]!.width = "56,";
    expect(draftMatches(d, sent())).toBe(false);
  });
});
