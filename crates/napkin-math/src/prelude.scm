;; The calculator's semantics.
;;
;; A napkin value is one of
;;   a number
;;   (money amount currency)
;;   (date days-from-the-common-era)
;;   (duration days)
;;
;; Every step returns (ok value) or (err "besked"), so a failure anywhere stops the
;; calculation and reaches the caller as a message rather than as an exception. That
;; matters because the caller is a keystroke handler: there is no stack to unwind to and
;; nothing useful to do with a condition object.
;;
;; The expression arrives already lowered: units have been resolved to money or duration
;; and `today` to a date, because the rate table and the clock live on the Rust side.
;; What is left here is the part that is genuinely a language, which is the dispatch of
;; the four operators across four kinds of value.

(define (ok v) (list 'ok v))
;; Not a result and not a mistake: a combination that more typing can still make sense
;; of. A number beside a date is the one that matters, because `today + 7` is what
;; `today + 7 days` looks like four keystrokes from the end.
(define (pending) (list 'pending))
(define (err m) (list 'err m))
(define (ok? r) (eq? (car r) 'ok))
(define (result-of r) (cadr r))

(define (bind r f) (if (ok? r) (f (result-of r)) r))
(define (bind2 a b f) (bind a (lambda (x) (bind b (lambda (y) (f x y))))))

(define (tagged? v t) (and (pair? v) (eq? (car v) t)))
(define (money? v) (tagged? v 'money))
(define (date? v) (tagged? v 'date))
(define (duration? v) (tagged? v 'duration))

(define (amount-of m) (cadr m))
(define (currency-of m) (caddr m))
(define (days-of v) (cadr v))

(define (make-money a c) (list 'money a c))
(define (make-date d) (list 'date d))
(define (make-duration d) (list 'duration d))

;; A date moves by whole days, so a fractional duration is rounded on the way in.
(define (whole d) (exact (round d)))

(define (rate-of rates c)
  (let ((hit (assq c rates)))
    (and hit (cdr hit))))

(define (unknown-currency c)
  (err (string-append "ukendt valuta: " (symbol->string c))))

(define (convert-money m to rates)
  (let ((from (currency-of m)))
    (if (eq? from to)
        (ok m)
        (let ((a (rate-of rates from))
              (b (rate-of rates to)))
          (cond
            ((not a) (unknown-currency from))
            ((not b) (unknown-currency to))
            (else (ok (make-money (* (/ (amount-of m) a) b) to))))))))

;; The target when none was named: kroner, unless it is already kroner, in which case
;; euros, because converting a thing into itself says nothing.
(define (implied-target m)
  (if (eq? (currency-of m) 'dkk) 'eur 'dkk))

(define (negate v)
  (cond
    ((number? v) (ok (- v)))
    ((money? v) (ok (make-money (- (amount-of v)) (currency-of v))))
    ((duration? v) (ok (make-duration (- (days-of v)))))
    (else (err "kan ikke vende en dato om"))))

(define (add a b rates)
  (cond
    ((and (number? a) (number? b)) (ok (+ a b)))
    ((and (money? a) (money? b))
     (bind (convert-money b (currency-of a) rates)
           (lambda (c) (ok (make-money (+ (amount-of a) (amount-of c)) (currency-of a))))))
    ((and (date? a) (duration? b)) (ok (make-date (+ (days-of a) (whole (days-of b))))))
    ((and (duration? a) (date? b)) (ok (make-date (+ (days-of b) (whole (days-of a))))))
    ((and (duration? a) (duration? b)) (ok (make-duration (+ (days-of a) (days-of b)))))
    ((and (date? a) (date? b)) (err "to datoer kan ikke lægges sammen"))
    ((or (and (date? a) (number? b)) (and (number? a) (date? b))) (pending))
    (else (err "de to ting kan ikke lægges sammen"))))

(define (sub a b rates)
  (cond
    ((and (number? a) (number? b)) (ok (- a b)))
    ((and (money? a) (money? b))
     (bind (convert-money b (currency-of a) rates)
           (lambda (c) (ok (make-money (- (amount-of a) (amount-of c)) (currency-of a))))))
    ((and (date? a) (duration? b)) (ok (make-date (- (days-of a) (whole (days-of b))))))
    ((and (date? a) (date? b)) (ok (make-duration (- (days-of a) (days-of b)))))
    ((and (duration? a) (duration? b)) (ok (make-duration (- (days-of a) (days-of b)))))
    ((or (and (date? a) (number? b)) (and (number? a) (date? b))) (pending))
    (else (err "de to ting kan ikke trækkes fra hinanden"))))

(define (mul a b rates)
  (cond
    ((and (number? a) (number? b)) (ok (* a b)))
    ((and (money? a) (number? b)) (ok (make-money (* (amount-of a) b) (currency-of a))))
    ((and (number? a) (money? b)) (ok (make-money (* a (amount-of b)) (currency-of b))))
    ((and (duration? a) (number? b)) (ok (make-duration (whole (* (days-of a) b)))))
    ((and (number? a) (duration? b)) (ok (make-duration (whole (* a (days-of b))))))
    (else (err "der kan kun ganges med et tal"))))

(define (divide a b rates)
  (cond
    ((and (number? b) (zero? b)) (err "division med nul"))
    ((and (number? a) (number? b)) (ok (/ a b)))
    ((and (money? a) (number? b)) (ok (make-money (/ (amount-of a) b) (currency-of a))))
    ((and (duration? a) (number? b)) (ok (make-duration (whole (/ (days-of a) b)))))
    ((and (money? a) (money? b))
     (bind (convert-money b (currency-of a) rates)
           (lambda (c)
             (if (zero? (amount-of c))
                 (err "division med nul")
                 (ok (/ (amount-of a) (amount-of c)))))))
    (else (err "der kan kun divideres med et tal"))))

(define (binary f a b rates)
  (bind2 (evaluate a rates) (evaluate b rates) (lambda (x y) (f x y rates))))

(define (evaluate expr rates)
  (cond
    ((number? expr) (ok expr))
    ((pair? expr)
     (let ((head (car expr)) (args (cdr expr)))
       (case head
         ((date) (ok (make-date (car args))))
         ((money)
          (bind (evaluate (car args) rates)
                (lambda (v)
                  (if (number? v)
                      (ok (make-money v (cadr args)))
                      (err "et beløb skal være et tal")))))
         ((duration)
          (bind (evaluate (car args) rates)
                (lambda (v)
                  (if (number? v)
                      (ok (make-duration (whole (* v (cadr args)))))
                      (err "en varighed skal være et tal")))))
         ((neg) (bind (evaluate (car args) rates) negate))
         ((convert)
          (bind (evaluate (car args) rates)
                (lambda (v)
                  (if (money? v)
                      (convert-money v (cadr args) rates)
                      (err "der kan kun omregnes beløb")))))
         ((+) (binary add (car args) (cadr args) rates))
         ((-) (binary sub (car args) (cadr args) rates))
         ((*) (binary mul (car args) (cadr args) rates))
         ((/) (binary divide (car args) (cadr args) rates))
         (else (err "ukendt udtryk")))))
    (else (err "ukendt værdi"))))

;; The entry point. `implicit` is true when the user did not say what to convert to,
;; which is the only time the calculator decides that for them.
(define (napkin-eval expr rates implicit)
  (let ((r (evaluate expr rates)))
    (if (and (ok? r) implicit (money? (result-of r)))
        (convert-money (result-of r) (implied-target (result-of r)) rates)
        r)))

napkin-eval
