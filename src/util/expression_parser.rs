pub enum ExpressionError {
    SyntaxError(String),
    ZeroDivision
}

enum Expr {
    Int(i64),
    UInt(u64),
    Float(f64),
    Var(String),
    UnaryNeg(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>)
}

enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow
}

struct ExpressionParser<'a> {
    text: &'a [u8],
    pos: usize
}

impl<'a> ExpressionParser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text: text.as_bytes(), pos: 0 }
    }

    /*fn parse(input: &str) -> Result<Expr, ExpressionError> {
        let mut p = Self::new(input);
        //let expr = 
        Ok(())
    }

    fn peek(&self) -> Option<&u8> {
        self.text.get(self.pos)
    }

    #[inline(always)]
    fn next(&mut self) {
        self.pos += 1;
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.next()
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ExpressionError> {
        let mut node = self.parse_term()?;
    }

    fn parse_variable(&mut self) -> Expr {
        
    }*/
}