// Which layout the window gets (web/DESIGN.md §2): from 1280 px the job and
// its plan share one workbench; below, they are two tabs.
const WIDE = "(min-width: 1280px)";

class Layout {
  wide = $state(matchMedia(WIDE).matches);

  constructor() {
    matchMedia(WIDE).addEventListener("change", (event) => {
      this.wide = event.matches;
    });
  }
}

export const layout = new Layout();
