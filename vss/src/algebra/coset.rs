use super::field::Field;

#[derive(Debug, Clone, Copy)]
struct Radix2Domain<T: Field> {
    order: usize,
    omega: T,
}

impl<T: Field> Radix2Domain<T> {
    pub fn new(order: usize, omega: T) -> Self {
        Radix2Domain { order, omega }
    }

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn omega(&self) -> T {
        self.omega
    }

    pub fn fft(&self, a: &mut Vec<T>) {
        _fft(a, self.omega);
    }

    pub fn ifft(&self, a: &mut Vec<T>) {
        _fft(a, self.omega.inverse());
        let t = T::from_int(self.order as u64).inverse();
        for i in a {
            *i *= t;
        }
    }

    pub fn coset_fft(&self, a: &mut Vec<T>, shift: T) {
        multiply_by_coset(a, shift);
        self.fft(a);
    }

    pub fn coset_ifft(&self, a: &mut Vec<T>, shift: T) {
        self.ifft(a);
        multiply_by_coset(a, shift.inverse());
    }
}

fn multiply_by_coset<T: Field>(a: &mut Vec<T>, shift: T) {
    let mut t = shift;
    for i in 1..a.len() {
        a[i] *= t;
        t *= shift;
    }
}

fn bitreverse(mut x: usize, len: usize) -> usize {
    let mut r = 0;
    for _i in 0..len {
        r = (r << 1) | (x & 1);
        x >>= 1;
    }
    r
}

fn _fft<T: Field>(a: &mut Vec<T>, omega: T) {
    let n = a.len();
    let log_n = n.ilog2() as usize;
    for i in 0..n {
        let rank = bitreverse(i, log_n);
        if i < rank {
            (a[i], a[rank]) = (a[rank], a[i]);
        }
    }
    let mut log_m = 0usize;
    for _i in 0..log_n {
        let w_m = omega.pow(n >> (log_m + 1));
        let m = 1 << log_m;
        for j in (0..n).step_by(m * 2) {
            let mut w = T::from_int(1);
            for k in 0..m {
                let t = w * a[j + k + m];
                a[j + k + m] = a[j + k] - t;
                a[j + k] += t;
                w *= w_m;
            }
        }
        log_m += 1;
    }
}

use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone)]
pub struct Coset<T: Field> {
    elements: Rc<RefCell<Vec<T>>>,
    elements_inv: Rc<RefCell<Vec<T>>>,
    fft_eval_domain: Radix2Domain<T>,
    shift: T,
}

impl<T: Field> Coset<T> {
    pub fn new(order: usize, shift: T) -> Self {
        assert!(!shift.is_zero());
        let omega = T::get_generator(order);
        Coset {
            elements: Rc::new(RefCell::new(vec![])),
            elements_inv: Rc::new(RefCell::new(vec![])),
            fft_eval_domain: Radix2Domain::new(order, omega),
            shift,
        }
    }

    pub fn order(&self) -> usize {
        self.fft_eval_domain.order
    }

    pub fn pow(&self, index: usize) -> Coset<T> {
        let lowbit = (index as i64 & (-(index as i64))) as usize;
        let omega = self.generator().pow(index);
        Coset {
            elements: Rc::new(RefCell::new(vec![])),
            elements_inv: Rc::new(RefCell::new(vec![])),
            fft_eval_domain: Radix2Domain::new(self.order() / lowbit, omega),
            shift: self.shift.pow(index),
        }
    }

    pub fn generator(&self) -> T {
        self.fft_eval_domain.omega
    }

    pub fn element_at(&self, index: usize) -> T {
        let mut elements = self.elements.borrow_mut();
        if elements.len() == 0 {
            let mut el = self.shift;
            let omega = self.generator();
            for _i in 0..self.fft_eval_domain.order() {
                elements.push(el);
                el *= omega;
            }
            elements[index]
        } else {
            elements[index]
        }
    }

    pub fn element_inv_at(&self, index: usize) -> T {
        let mut elements_inv = self.elements_inv.borrow_mut();
        if elements_inv.len() == 0 {
            let mut el = self.shift.inverse();
            let omega_inv = self.generator().pow(self.order() - 1);
            for _i in 0..self.fft_eval_domain.order() {
                elements_inv.push(el);
                el *= omega_inv;
            }
            elements_inv[index]
        } else {
            elements_inv[index]
        }
    }

    pub fn all_elements_inv(&self) -> Vec<T> {
        let mut elements_inv = self.elements_inv.borrow_mut();
        if elements_inv.len() == 0 {
            let mut el = self.shift.inverse();
            let omega_inv = self.generator().pow(self.order() - 1);
            for _i in 0..self.fft_eval_domain.order() {
                elements_inv.push(el);
                el *= omega_inv;
            }
        }
        elements_inv.clone()
    }

    pub fn all_elements(&self) -> Vec<T> {
        let mut elements = self.elements.borrow_mut();
        if elements.len() == 0 {
            let mut el = self.shift;
            for _i in 0..self.fft_eval_domain.order() {
                elements.push(el);
                el *= self.fft_eval_domain.omega();
            }
        }
        elements.clone()
    }

    pub fn size(&self) -> usize {
        self.fft_eval_domain.order()
    }

    pub fn fft(&self, coeff: &Vec<T>) -> Vec<T> {
        let mut a = coeff.clone();
        let n = self.size() - a.len();
        for _i in 0..n {
            a.push(T::from_int(0));
        }
        self.fft_eval_domain.coset_fft(&mut a, self.shift);
        a
    }

    pub fn ifft(&self, evals: &Vec<T>) -> Vec<T> {
        if evals.len() == 1 {
            return vec![evals[0]];
        };
        let mut a = evals.clone();
        self.fft_eval_domain.coset_ifft(&mut a, self.shift);
        a
    }

    pub fn shift(&self) -> T {
        self.shift
    }
}

#[cfg(test)]
mod tests {
    use crate::algebra::field::mersenne61_ext::Mersenne61Ext;

    use super::super::field::fp64::Fp64;
    use super::*;
    use rand::Rng;

    #[test]
    fn fft_and_ifft() {
        let mut a: Vec<Fp64> = vec![];
        let mut b: Vec<Fp64> = vec![];
        for _i in 0..16 {
            a.push(Fp64::random_element());
            b.push(Fp64::random_element());
        }
        for _i in 16..32 {
            a.push(Fp64::from_int(0));
            b.push(Fp64::from_int(0));
        }
        let mut a_times_b: Vec<Fp64> = vec![];
        for _i in 0..32 {
            a_times_b.push(Fp64::from_int(0));
        }
        for i in 0..16usize {
            for j in 0..16usize {
                a_times_b[i + j] += a[i] * b[j];
            }
        }
        let domain: Radix2Domain<Fp64> = Radix2Domain::new(32, Fp64::get_generator(32));
        domain.fft(&mut a);
        domain.fft(&mut b);
        for i in 0..a.len() {
            a[i] *= b[i];
        }
        domain.ifft(&mut a);
        assert_eq!(&a, &a_times_b);
        let shift = Fp64::random_element();
        domain.coset_fft(&mut a, shift);
        domain.coset_ifft(&mut a, shift);
        assert_eq!(&a, &a_times_b);
    }

    #[test]
    fn all_elements() {
        let r = Fp64::random_element();
        let coset = Coset::new(32, r);
        let elements = coset.all_elements();
        assert_eq!(elements[0], r);
        let omega = coset.fft_eval_domain.omega();
        for i in 0..elements.len() - 1 {
            assert_eq!(elements[i] * omega, elements[i + 1]);
        }
        assert_eq!(*elements.last().unwrap() * omega, elements[0]);
    }

    #[test]
    fn pow() {
        let shift = Mersenne61Ext::random_element();
        let coset = Coset::new(32, shift);
        let coset_square = coset.pow(2);
        for (idx, i) in coset_square.all_elements().iter().enumerate() {
            assert_eq!(*i, coset.all_elements()[idx].pow(2));
            assert_eq!(*i, coset.all_elements()[idx + coset_square.size()].pow(2));
        }
        let coset_exp6 = coset.pow(12);
        for (idx, i) in coset.all_elements().iter().enumerate() {
            assert_eq!(
                i.pow(12),
                coset_exp6.all_elements()[idx % coset_exp6.size()]
            );
        }
        let r = rand::thread_rng().gen();
        let coset_rand = coset.pow(r);
        for (idx, i) in coset.all_elements().iter().enumerate() {
            assert_eq!(i.pow(r), coset_rand.all_elements()[idx % coset_rand.size()]);
        }
    }
}