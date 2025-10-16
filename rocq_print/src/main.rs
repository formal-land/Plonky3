use alloc::rc::Rc;
use p3_keccak_air::KeccakAir;
use core::fmt::{Debug};
use p3_air::{AirBuilder, Air, LoggingAirBuilder};
use p3_field::Field;
use p3_goldilocks::Goldilocks;
use p3_matrix::dense::RowMajorMatrix;
use p3_uni_stark::{Entry, SymbolicExpression, SymbolicVariable};

extern crate alloc;

enum Constraint<E> {
    AssertZero(E),
    Message(String),
}

struct RocqAirBuilder {
    main: RowMajorMatrix<SymbolicVariable<Goldilocks>>,
    constraints: Vec<Constraint<SymbolicExpression<Goldilocks>>>,
}

impl RocqAirBuilder {
    fn new(width: usize, height: usize) -> Self {
        let mut main = RowMajorMatrix::new(
            vec![SymbolicVariable::new(Entry::Public, 0); width * height],
            width,
        );

        for h in 0..height {
            for w in 0..width {
                main.values[h * width + w] = SymbolicVariable::new(Entry::Public, h * width + w);
            }
        }

        Self {
            main,
            constraints: Vec::new(),
        }
    }
}

impl AirBuilder for RocqAirBuilder {
    type F = Goldilocks;

    type Expr = SymbolicExpression<Self::F>;

    type Var = SymbolicVariable<Self::F>;

    type M = RowMajorMatrix<Self::Var>;

    fn main(&self) -> <Self as AirBuilder>::M {
        self.main.clone()
    }

    fn is_first_row(&self) -> <Self as AirBuilder>::Expr {
        SymbolicExpression::IsFirstRow
    }

    fn is_last_row(&self) -> <Self as AirBuilder>::Expr {
        SymbolicExpression::IsLastRow
    }

    fn is_transition_window(&self, _: usize) -> <Self as AirBuilder>::Expr {
        SymbolicExpression::IsTransition
    }

    fn assert_zero<I>(&mut self, expr: I)
    where
        I: Into<Self::Expr>,
    {
        self.constraints.push(Constraint::AssertZero(expr.into()));
    }
}

impl LoggingAirBuilder for RocqAirBuilder {
    fn log_in_constraints(&mut self, message: &str) {
        self.constraints
            .push(Constraint::Message(message.to_string()));
    }
}

pub(crate) fn print_keccak() {
    let mut builder = RocqAirBuilder::new(
        p3_keccak_air::NUM_KECCAK_COLS,
        2,
    );

    (KeccakAir {}).eval(&mut builder);

    builder.to_rocq(0);
    // println!("{:?}", builder.constraints.len());
    println!("Result 🛍️");
    println!("  tt");
}

trait ToRocq {
    fn to_rocq(&self, indent: usize);
}

enum FlatSymbolicExpression<'a, F> {
    Variable(&'a SymbolicVariable<F>),
    IsFirstRow,
    IsLastRow,
    IsTransition,
    Constant(F),
    Add(Vec<Rc<FlatSymbolicExpression<'a, F>>>),
    Sub {
        x: Rc<FlatSymbolicExpression<'a, F>>,
        y: Rc<FlatSymbolicExpression<'a, F>>,
    },
    Neg {
        x: Rc<FlatSymbolicExpression<'a, F>>,
    },
    Mul(Vec<Rc<FlatSymbolicExpression<'a, F>>>),
}

impl<'a, F> FlatSymbolicExpression<'a, F>
where
    F: Field,
{
    fn size(&self) -> usize {
        match self {
            FlatSymbolicExpression::Variable(_) => 1,
            FlatSymbolicExpression::IsFirstRow => 1,
            FlatSymbolicExpression::IsLastRow => 1,
            FlatSymbolicExpression::IsTransition => 1,
            FlatSymbolicExpression::Constant(_) => 1,
            FlatSymbolicExpression::Add(xs) => xs.iter().map(|x| x.size()).sum(),
            FlatSymbolicExpression::Sub { x, y } => x.size() + y.size(),
            FlatSymbolicExpression::Neg { x } => x.size(),
            FlatSymbolicExpression::Mul(xs) => xs.iter().map(|x| x.size()).sum(),
        }
    }

    fn from_symbolic_expression(expr: &'a SymbolicExpression<F>) -> Self {
        match expr {
            SymbolicExpression::Variable(v) => Self::Variable(v),
            SymbolicExpression::IsFirstRow => Self::IsFirstRow,
            SymbolicExpression::IsLastRow => Self::IsLastRow,
            SymbolicExpression::IsTransition => Self::IsTransition,
            SymbolicExpression::Constant(c) => Self::Constant(*c),
            SymbolicExpression::Add { x, y, .. } => {
                let x = Self::from_symbolic_expression(x);
                let y = Self::from_symbolic_expression(y);
                match (&x, &y) {
                    (Self::Add(xs), Self::Add(ys)) => {
                        Self::Add([xs.to_vec(), ys.to_vec()].concat())
                    }
                    (Self::Add(xs), _) => Self::Add([xs.to_vec(), vec![Rc::new(y)]].concat()),
                    (_, Self::Add(ys)) => Self::Add([vec![Rc::new(x)], ys.to_vec()].concat()),
                    (_, _) => Self::Add(vec![Rc::new(x), Rc::new(y)]),
                }
            }
            SymbolicExpression::Sub { x, y, .. } => Self::Sub {
                x: Rc::new(Self::from_symbolic_expression(x)),
                y: Rc::new(Self::from_symbolic_expression(y)),
            },
            SymbolicExpression::Neg { x, .. } => Self::Neg {
                x: Rc::new(Self::from_symbolic_expression(x)),
            },
            SymbolicExpression::Mul { x, y, .. } => {
                let x = Self::from_symbolic_expression(x);
                let y = Self::from_symbolic_expression(y);
                match (&x, &y) {
                    (Self::Mul(xs), Self::Mul(ys)) => {
                        Self::Mul([xs.to_vec(), ys.to_vec()].concat())
                    }
                    (Self::Mul(xs), _) => Self::Mul([xs.to_vec(), vec![Rc::new(y)]].concat()),
                    (_, Self::Mul(ys)) => Self::Mul([vec![Rc::new(x)], ys.to_vec()].concat()),
                    (_, _) => Self::Mul(vec![Rc::new(x), Rc::new(y)]),
                }
            }
        }
    }
}

impl<'a, F> ToRocq for FlatSymbolicExpression<'a, F>
where
    F: Debug,
{
    fn to_rocq(&self, indent: usize) {
        match self {
            FlatSymbolicExpression::Variable(v) => {
                println!("{}{} {:?}", " ".repeat(indent), "Variable:", v.index);
            }
            FlatSymbolicExpression::IsFirstRow => {
                println!("{}{}", " ".repeat(indent), "IsFirstRow");
            }
            FlatSymbolicExpression::IsLastRow => {
                println!("{}{}", " ".repeat(indent), "IsLastRow");
            }
            FlatSymbolicExpression::IsTransition => {
                println!("{}{}", " ".repeat(indent), "IsTransition");
            }
            FlatSymbolicExpression::Constant(c) => {
                println!("{}{} {:?}", " ".repeat(indent), "Constant:", c);
            }
            FlatSymbolicExpression::Add(xs) => {
                println!("{}{}", " ".repeat(indent), "Add:");
                for x in xs {
                    x.to_rocq(indent + 2);
                }
            }
            FlatSymbolicExpression::Sub { x, y } => {
                println!("{}{}", " ".repeat(indent), "Sub:");
                x.to_rocq(indent + 2);
                y.to_rocq(indent + 2);
            }
            FlatSymbolicExpression::Neg { x } => {
                println!("{}{}", " ".repeat(indent), "Neg:");
                x.to_rocq(indent + 2);
            }
            FlatSymbolicExpression::Mul(xs) => {
                println!("{}{}", " ".repeat(indent), "Mul:");
                for x in xs {
                    x.to_rocq(indent + 2);
                }
            }
        }
    }
}

impl<F> ToRocq for SymbolicExpression<F>
where
    F: Field,
    F: Debug,
{
    fn to_rocq(&self, indent: usize) {
        FlatSymbolicExpression::from_symbolic_expression(self).to_rocq(indent);
    }
}

impl<T: ToRocq> ToRocq for Option<T> {
    fn to_rocq(&self, indent: usize) {
        match self {
            Some(t) => {
                println!("{}{}", " ".repeat(indent), "Some:");
                t.to_rocq(indent + 2);
            }
            None => {
                println!("{}{}", " ".repeat(indent), "None");
            }
        }
    }
}

impl<T: ToRocq, const N: usize> ToRocq for [T; N] {
    fn to_rocq(&self, indent: usize) {
        println!("{}{}", " ".repeat(indent), "Array:");
        for item in self {
            item.to_rocq(indent + 2);
        }
    }
}

impl ToRocq for RocqAirBuilder {
    fn to_rocq(&self, indent: usize) {
        println!("{}{}", " ".repeat(indent), "Trace 🐾");
        // let mut i: u32 = 0;
        for item in &self.constraints {
            // i += 1;
            // eprintln!("i = {}", i);
            match item {
                Constraint::AssertZero(expr) => {
                    println!("{}{}", " ".repeat(indent + 2), "AssertZero:");
                    expr.to_rocq(indent + 4);
                    // eprintln!("size = {}", FlatSymbolicExpression::from_symbolic_expression(expr).size());
                }
                Constraint::Message(message) => {
                    println!("{}{}", " ".repeat(indent + 2), "Message 🦜");
                    println!("{}{}", " ".repeat(indent + 4), message);
                }
            }
        }
    }
}

fn main() {
    print_keccak();
}
