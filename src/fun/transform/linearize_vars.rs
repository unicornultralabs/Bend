use crate::{
  fun::{Book, FanKind, Name, Pattern, Tag, Term},
  maybe_grow,
};
use std::collections::HashMap;

/// Erases variables that weren't used, dups the ones that were used more than once.
/// Substitutes lets into their variable use.
/// In details:
/// For all var declarations:
///   If they're used 0 times: erase the declaration
///   If they're used 1 time: leave them as-is
///   If they're used more times: insert dups to make var use affine
/// For all let vars:
///   If they're used 0 times: Discard the let
///   If they're used 1 time: substitute the body in the var use
///   If they're use more times: add dups for all the uses, put the let body at the root dup.
/// Precondition: All variables are bound and have unique names within each definition.
impl Book {
  pub fn linearize_vars(&mut self) {
    for def in self.defs.values_mut() {
      def.rule_mut().body.linearize_vars();
    }
  }
}

impl Term {
  pub fn linearize_vars(&mut self) {
    term_to_linear(self, &mut HashMap::new());
  }
}

fn term_to_linear(term: &mut Term, var_uses: &mut HashMap<Name, u64>) {
  maybe_grow(|| {
    if let Term::Let { pat, val, nxt } = term {
      if let Pattern::Var(Some(nam)) = pat.as_ref() {
        // TODO: This is swapping the order of how the bindings are
        // used, since it's not following the usual AST order (first
        // val, then nxt). Doesn't change behaviour, but looks strange.
        term_to_linear(nxt, var_uses);

        let uses = get_var_uses(Some(nam), var_uses);
        term_to_linear(val, var_uses);
        match uses {
          0 => {
            let Term::Let { pat, .. } = term else { unreachable!() };
            **pat = Pattern::Var(None);
          }
          1 => {
            nxt.subst(nam, val.as_ref());
            *term = std::mem::take(nxt.as_mut());
          }
          _ => {
            let new_pat = duplicate_pat(nam, uses);
            let Term::Let { pat, .. } = term else { unreachable!() };
            *pat = new_pat;
          }
        }
        return;
      }
    }
    if let Term::Var { nam } = term {
      let instantiated_count = var_uses.entry(nam.clone()).or_default();
      *instantiated_count += 1;
      *nam = dup_name(nam, *instantiated_count);
      return;
    }

    for (child, binds) in term.children_mut_with_binds_mut() {
      term_to_linear(child, var_uses);

      for bind in binds {
        let uses = get_var_uses(bind.as_ref(), var_uses);
        match uses {
          // Erase binding
          0 => *bind = None,
          // Keep as-is
          1 => (),
          // Duplicate binding
          uses => duplicate_term(bind.as_ref().unwrap(), child, uses, None),
        }
      }
    }
  })
}

fn get_var_uses(nam: Option<&Name>, var_uses: &HashMap<Name, u64>) -> u64 {
  nam.and_then(|nam| var_uses.get(nam).copied()).unwrap_or_default()
}

/// Creates the duplication bindings for variables used multiple times.
///
/// Example:
///
/// `@x (x x x x)` becomes `@x let {x0 x1 x2 x3} = x; (x0 x1 x2 x3)`.
///
/// `let x = @y y; (x x x)` becomes `let {x0 x1 x2} = @y y; (x0 x1 x2)`.
fn duplicate_term(nam: &Name, nxt: &mut Term, uses: u64, dup_body: Option<&mut Term>) {
  debug_assert!(uses > 1);

  *nxt = Term::Let {
    pat: duplicate_pat(nam, uses),
    val: Box::new(dup_body.map_or_else(|| Term::Var { nam: nam.clone() }, std::mem::take)),
    nxt: Box::new(std::mem::take(nxt)),
  }
}

fn duplicate_pat(nam: &Name, uses: u64) -> Box<Pattern> {
  Box::new(Pattern::Fan(
    FanKind::Dup,
    Tag::Auto,
    (1..uses + 1).map(|i| Pattern::Var(Some(dup_name(nam, i)))).collect(),
  ))
}

fn dup_name(nam: &Name, uses: u64) -> Name {
  if uses == 1 {
    nam.clone()
  } else {
    Name::new(format!("{nam}_{uses}"))
  }
}
