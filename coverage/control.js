
function next_uncovered(selector, reverse, scroll_selector) {
  function visit_element(element) {
    element.classList.add("seen");
    element.classList.add("selected");

    if (!scroll_selector) {
      scroll_selector = "tr:has(.selected) td.line-number"
    }

    const scroll_to = document.querySelector(scroll_selector);
    if (scroll_to) {
      scroll_to.scrollIntoView({behavior: "smooth", block: "center", inline: "end"});
    }
  }

  function select_one() {
    if (!reverse) {
      const previously_selected = document.querySelector(".selected");

      if (previously_selected) {
        previously_selected.classList.remove("selected");
      }

      return document.querySelector(selector + ":not(.seen)");
    } else {
      const previously_selected = document.querySelector(".selected");

      if (previously_selected) {
        previously_selected.classList.remove("selected");
        previously_selected.classList.remove("seen");
      }

      const nodes = document.querySelectorAll(selector + ".seen");
      if (nodes) {
        const last = nodes[nodes.length - 1]; // last
        return last;
      } else {
        return undefined;
      }
    }
  }

  function reset_all() {
    if (!reverse) {
      const all_seen = document.querySelectorAll(selector + ".seen");

      if (all_seen) {
        all_seen.forEach(e => e.classList.remove("seen"));
      }
    } else {
      const all_seen = document.querySelectorAll(selector + ":not(.seen)");

      if (all_seen) {
        all_seen.forEach(e => e.classList.add("seen"));
      }
    }

  }

  const uncovered = select_one();

  if (uncovered) {
    visit_element(uncovered);
  } else {
    reset_all();

    const uncovered = select_one();

    if (uncovered) {
      visit_element(uncovered);
    }
  }
}

function next_line(reverse) {
  next_uncovered("td.uncovered-line", reverse)
}

function next_region(reverse) {
  next_uncovered("span.red.region", reverse);
}

function next_branch(reverse) {
  next_uncovered("span.red.branch", reverse);
}

document.addEventListener("keypress", function(event) {
  const reverse = event.shiftKey;
  if (event.code == "KeyL") {
    next_line(reverse);
  }
  if (event.code == "KeyB") {
    next_branch(reverse);
  }
  if (event.code == "KeyR") {
    next_region(reverse);
  }
});
