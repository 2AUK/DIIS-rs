use ndarray::{Array, Array1, Array2, Ix1};
use ndarray_linalg::*;
use std::iter::FromIterator;
use std::collections::VecDeque;
use num::Float;
use crate::solver::*;
use crate::core::*;
use crate::types::*;

pub struct DIIS<F, T: ConvProblem> {
    pub depth: usize,
    pub eta: F,
    pub restart: i64,
    memory_vec: VecDeque<Array<T::Elem, T::Dim>>,
    residuals_vec: VecDeque<Array<T::Elem, T::Dim>>,
    rms_vec: VecDeque<F>,
}

impl<F: ConvFloat, T: ConvProblem<Elem = F>> DIIS<F, T> {
    pub fn new(eta: F, depth: usize, restart: i64) -> Self {
        DIIS {
            eta,
            depth,
            restart,
            memory_vec: VecDeque::new(),
            residuals_vec: VecDeque::new(),
            rms_vec: VecDeque::new(),
        }
    }

    fn linear_mixing_step(&mut self, prev: &Array<T::Elem, T::Dim>, curr: &Array<T::Elem, T::Dim>) -> Array<T::Elem, T::Dim> {
        let out = curr * self.eta + (prev * (F::from_f64(1.0).unwrap() - self.eta));
        self.memory_vec.push_back(curr.clone());
        self.residuals_vec.push_back(curr.clone()-prev.clone());
        out
    }

    fn diis_step(&mut self, prev: &Array<T::Elem, T::Dim>, curr: &Array<T::Elem, T::Dim>) -> Array<T::Elem, T::Dim> {
        let mut A: Array2<T::Elem> = Array2::from_elem((self.depth+1, self.depth+1), F::from_f64(-1.0).unwrap());
        let mut b: Array1<T::Elem> = Array1::zeros(self.depth+1);
        let mut c_A: Array<T::Elem, T::Dim> = Array::zeros(self.memory_vec[0].dim());
        let mut min_res: Array<T::Elem, T::Dim> = Array::zeros(self.memory_vec[0].dim());

        b[self.depth] = F::from_f64(-1.0).unwrap();

        A[[self.depth, self.depth]] = F::from_f64(0.0).unwrap();
        let res = Array::from_vec(Vec::from_iter(self.residuals_vec.iter().cloned()));
        for i in 1..self.depth {
            for j in 1..self.depth{
                let resi = Array::from_iter(res[i].iter().cloned());
                let resj = Array::from_iter(res[j].iter().cloned());
                A[[i, j]] = resi.dot(&resj);
            }
        }
        let coef = A.solve(&b).unwrap();

        let fr: Array<Array<T::Elem, T::Dim>, Ix1> = Array::from_vec(
            Vec::from_iter(
                self.memory_vec.iter().cloned()
            ));

        for (i, arr) in self.memory_vec.iter().cloned().enumerate() {
            c_A += &(arr * coef[i]);
            min_res += &(self.residuals_vec[i].clone() * coef[i]);
        }

        let c_new = min_res * self.eta + c_A;

        self.memory_vec.push_back(curr.clone());
        self.residuals_vec.push_back(curr.clone()-prev.clone());

        self.memory_vec.pop_front().unwrap();
        self.residuals_vec.pop_front().unwrap();

        c_new
    }

    fn compute_rms(&mut self, prev: &Array<T::Elem, T::Dim>, curr: &Array<T::Elem, T::Dim>) -> T::Elem {
        let prefac = F::from_f64(1.0 / curr.len() as f64).unwrap();
        let curr_minus_prev = (curr - prev).sum();
        let result = prefac * Float::powf(curr_minus_prev, F::from_f64(2.0).unwrap());

        result
    }
}

impl<T, F> Converger<T> for DIIS<F, T>
where
    F: ConvFloat,
    T: ConvProblem<Elem = F>,
{
    fn next_iter(&mut self, problem: &mut T, state: &mut ConvState<T>) -> Array<T::Elem, T::Dim> {
        let prev = state.input.clone();
        let curr = problem.update(&prev);
        if self.memory_vec.len() < self.depth {
            let out = self.linear_mixing_step(&prev, &curr);
            let rms = self.compute_rms(&prev, &out);
            self.rms_vec.push_back(rms);
            return out;
        } else {
            let mut out = self.diis_step(&prev, &curr);
            let rms = self.compute_rms(&prev, &out);
            let min_val  = self.rms_vec.iter().fold(F::infinity(), |a, &b| a.min(b));
            let min_index = self.rms_vec.iter().position(|&x| x == min_val).unwrap();

            if rms > F::from_i64(self.restart).unwrap() * min_val {
                out = self.memory_vec[min_index].clone();
                self.memory_vec.clear();
                self.residuals_vec.clear();
                self.rms_vec.clear();
            }
            self.rms_vec.push_back(rms);
            self.rms_vec.pop_front();
            return out;
        }
    }
}
