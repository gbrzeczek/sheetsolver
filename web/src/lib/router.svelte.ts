// Hash routes (web/DESIGN.md §2, "URLs"). They never reach the server, so
// they cannot clash with the API's paths.

export type Route =
  | { name: "setup" }
  | { name: "plan"; id: string; panel: number | null; cut: number | null };

export function parse(hash: string): Route {
  const [path = "", query = ""] = hash.replace(/^#/, "").split("?");
  const params = new URLSearchParams(query);
  const m = /^\/jobs\/([^/]+)(?:\/panels\/(\d+))?\/?$/.exec(path);
  if (!m) return { name: "setup" };
  const cut = Number(params.get("cut"));
  return {
    name: "plan",
    id: decodeURIComponent(m[1]!),
    panel: m[2] ? Number(m[2]) : null,
    cut: Number.isInteger(cut) && cut > 0 ? cut : null,
  };
}

export function href(route: Route): string {
  if (route.name === "setup") return "#/";
  let h = `#/jobs/${encodeURIComponent(route.id)}`;
  if (route.panel !== null) h += `/panels/${route.panel}`;
  if (route.panel !== null && route.cut !== null) h += `?cut=${route.cut}`;
  return h;
}

class Router {
  route = $state<Route>(parse(location.hash));

  constructor() {
    window.addEventListener("hashchange", () => {
      this.route = parse(location.hash);
    });
  }

  go(route: Route, replace = false): void {
    const h = href(route);
    if (replace) history.replaceState(null, "", h);
    else history.pushState(null, "", h);
    this.route = route;
  }
}

export const router = new Router();
