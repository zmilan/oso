use std::collections::HashSet;

use crate::folder::{fold_operation, Folder};
use crate::formatting::ToPolarString;
use crate::kb::Bindings;
use crate::terms::{Operation, Operator, Symbol, Term, TermList, Value};

// Variable(?) <= bound value which might be a partial
//
// Top level unify
//
// a: _this = ?
// ?
//
// Dot op and comparison or unify
//
// a: (_this.foo = _temp)
// _temp: this = ?
//
// a: _this.foo = _temp
// _temp: this > 0
//
// a: _this.foo = _temp
// _temp: this > 0, this = 1, this < 0
//
// _this.foo > 0 and _this.foo = 1 and _this.foo < 0
//
// a: _this.a = _value_2_8
// _value_2_8: _this.b = _value_1_9
// _value_1_9: _this > 0
//
// a: _this.a.b = _value_1_9
// _value_1_9: _this > 0
//
// a: _this.a.b > 0

pub fn simplify_bindings(mut bindings: Bindings) -> Bindings {
    let root_partials = get_roots(&bindings);

    for root in root_partials.iter() {
        let simplified = simplify_partial(bindings.get(root).unwrap().clone());
        bindings.insert(root.clone(), simplified);
    }

    to_expressions(&mut bindings);
    remove_temporaries(&mut bindings);

    bindings
}

pub struct Simplifier;

// Unify(Partial(Gt(_this, 0), Lt(_this, 4)), Dot(_this, a))

//

// And(Comparison(Dot, Number), Comparison(Dot, Number))
// And(Gt(_this.a, 0), Lt(_this.a, 4))

// partial(_this = ?) => ?
impl Folder for Simplifier {
    fn fold_operation(&mut self, o: Operation) -> Operation {
        fn sub_this(term: &Term, replacement: &Term) -> Term {
            match (term.value(), replacement.value()) {
                (
                    Value::Expression(Operation {
                        operator: Operator::Dot,
                        args,
                    }),
                    Value::Expression(Operation {
                        operator: Operator::Dot,
                        ..
                    }),
                ) => term.clone_with_value(Value::Expression(Operation {
                    operator: Operator::Dot,
                    args: vec![replacement.clone(), args.get(1).unwrap().clone()],
                })),
                _ => {
                    if is_this_arg(term.value()) {
                        replacement.clone()
                    } else {
                        term.clone()
                    }
                }
            }
        }

        let mut map_ops = |ops: &[Operation], replacement: &Term| -> TermList {
            eprintln!(
                "MAP_OPS\n\tOPS: {:?}\n\tREPLACEMENT: {}",
                ops.iter().map(|o| o.to_polar()).collect::<Vec<String>>(),
                replacement.to_polar()
            );
            ops.iter()
                .map(|o| Operation {
                    operator: o.operator,
                    args: o.args.iter().map(|a| sub_this(a, replacement)).collect(),
                })
                .map(|o| replacement.clone_with_value(Value::Expression(fold_operation(o, self))))
                .collect()
        };

        match o.operator {
            Operator::Unify => {
                let left = o.args.get(0).unwrap();
                let right = o.args.get(1).unwrap();
                eprintln!("LEFT: {}\nRIGHT: {}", left.to_polar(), right.to_polar());
                Operation {
                    operator: Operator::And,
                    args: match (left.value(), right.value()) {
                        (Value::Partial(c), Value::Expression(_)) => {
                            eprintln!(
                                "PARTIAL: {}\n\tEXPRESSION: {})",
                                left.to_polar(),
                                right.to_polar()
                            );
                            map_ops(&c.operations, right)
                        }
                        (Value::Expression(_), Value::Partial(c)) => {
                            eprintln!("(EXPRESSION, PARTIAL)");
                            map_ops(&c.operations, left)
                        }
                        _ => return fold_operation(o, self),
                    },
                }
            }
            _ => fold_operation(o, self),
        }
    }
}

fn simplify_partial(mut term: Term) -> Term {
    let mut simplifier = Simplifier {};
    let mut new;
    loop {
        new = simplifier.fold_term(term.clone());
        if new == term {
            break;
        }
        term = new;
    }
    new
}

fn is_this_arg(value: &Value) -> bool {
    match value {
        Value::Expression(Operation {
            operator: Operator::Dot,
            args,
        }) => {
            eprintln!("is_this_arg({}) -> true", value.to_polar());
            assert!(
                matches!(args.get(0).unwrap().value(), Value::Variable(sym) if sym.0 == "_this")
            );
            true
        }
        Value::Variable(Symbol(name)) if name == "_this" => {
            eprintln!("is_this_arg({}) -> true", value.to_polar());
            true
        }
        _ => {
            eprintln!("is_this_arg({}) -> false", value.to_polar());
            false
        }
    }
}

// partial(_x_5) { partial(_value_1_6) { _this > 0, _this > 1 } = _this.a }

fn get_roots(bindings: &Bindings) -> HashSet<Symbol> {
    let mut roots = HashSet::new();
    for (symbol, val) in bindings.iter() {
        if !symbol.is_temporary_var() {
            if let Value::Partial(_) = val.value() {
                roots.insert(symbol.clone());
            }
        }
    }

    roots
}

fn to_expressions(bindings: &mut Bindings) {
    let mut new_bindings = Bindings::new();

    for (name, val) in bindings.iter() {
        if let Value::Partial(partial) = val.value() {
            let name = name.clone();
            let partial = partial.clone().into_expression();
            new_bindings.insert(name, partial);
        }
    }

    bindings.extend(new_bindings.into_iter());
}

fn remove_temporaries(bindings: &mut Bindings) {
    let mut remove = HashSet::new();

    for (name, _) in bindings.iter() {
        if name.is_temporary_var() {
            remove.insert(name.clone());
        }
    }

    for name in remove.iter() {
        bindings.remove(name);
    }
}
