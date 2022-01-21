use log::{debug, error};
use std::collections::hash_map::*;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, Write};
use structopt::StructOpt;

const LETTER_FREQ: [(char, f64); 26] = [
    ('e', 0.111607),
    ('a', 0.084966),
    ('r', 0.075809),
    ('i', 0.075448),
    ('o', 0.071635),
    ('t', 0.069509),
    ('n', 0.066544),
    ('s', 0.057351),
    ('l', 0.054893),
    ('c', 0.045388),
    ('u', 0.036308),
    ('d', 0.033844),
    ('p', 0.031671),
    ('m', 0.030129),
    ('h', 0.030034),
    ('g', 0.024705),
    ('b', 0.020720),
    ('f', 0.018121),
    ('y', 0.017779),
    ('w', 0.012899),
    ('k', 0.011016),
    ('v', 0.010074),
    ('x', 0.002902),
    ('z', 0.002722),
    ('j', 0.001965),
    ('q', 0.001962),
];

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(default_value = "5")]
    num_letters: usize,

    #[structopt(default_value = "/usr/share/dict/words")]
    dictionary_path: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Info {
    /// Green letters
    Exact(char),

    /// Yellow letters
    Somewhere(char),

    /// Gray letters
    No(char),
}

#[derive(Debug, Clone)]
struct Knowledge {
    restrictions: Vec<Restriction>,
    must_have: HashMap<char, usize>,
}

#[derive(Debug, Clone)]
enum Restriction {
    Exact(char),
    Not(Vec<char>),
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args = Args::from_args();

    let mut knowledge = Knowledge::new(args.num_letters);

    let mut words_file = match File::open(&args.dictionary_path) {
        Ok(f) => f,
        Err(e) => {
            error!("dictionary file {:?} could not be opened: {}", args.dictionary_path, e);
            error!("to use a different file, specify it in command line arguments");
            Args::clap().print_help().unwrap();
            println!();
            std::process::exit(1);
        }
    };

    loop {
        let mut candidates = vec![];
        for res in BufReader::new(&mut words_file).lines() {
            let word = res?;

            if knowledge.check_word(&word) {
                debug!("adding {}", word);
                candidates.push(word);
            }
        }

        if candidates.is_empty() {
            println!("no candidates left!");
            return Ok(());
        }

        print_words("candidates", candidates.iter());

        let mut by_letters = candidates
            .iter()
            .map(|word| {
                let mut letters = word.chars().collect::<Vec<_>>();
                letters.sort_unstable();
                letters.dedup();
                (word.clone(), letters.len())
            })
            .collect::<Vec<_>>();
        by_letters.sort_unstable_by(|(_word1, count1), (_word2, count2)| count2.cmp(count1));

        let most_unique_letters = by_letters.split(|(_word, count)| *count < by_letters[0].1).next().unwrap();

        if most_unique_letters.len() > 1 {
            let mut by_freq = most_unique_letters
                .iter()
                .map(|(word, _count)| {
                    (word, word.chars()
                        .map(|c| {
                            // Letters we already have knowledge about count for zero.
                            if knowledge.must_have.iter().any(|(&x, _)| x == c)
                                || knowledge.restrictions.iter().any(|r| {
                                    match r {
                                        Restriction::Not(v) => v.iter().any(|&x| x == c),
                                        Restriction::Exact(x) => *x == c,
                                    }
                                })
                            {
                                0.
                            } else {
                                // Otherwise add up the English letter frequency
                                LETTER_FREQ.iter().find(|(l, _f)| *l == c).unwrap().1
                            }
                        })
                        .sum::<f64>())
                })
                .collect::<Vec<_>>();
            by_freq.sort_unstable_by(|(_word1, score1), (_word2, score2)| {
                use std::cmp::Ordering::*;
                if score1 > score2 {
                    Less
                } else {
                    Greater
                }
            });
            print_words("most unique letters, sorted by letter frequency",
                by_freq.iter().map(|(word, score)| format!("\n\t({}, {})", word, score)));
        } else {
            println!("most unique letters: {}", most_unique_letters[0].0);
        }

        loop {
            print!("Type the guess you made. Prefix each letter with: green=*, yellow=?, gray=!: ");
            io::stdout().flush()?;
            let mut inp = String::new();
            io::stdin().read_line(&mut inp)?;
            inp = inp.trim().to_owned();
            if inp.is_empty() {
                return Ok(());
            }
            match parse_input(&inp, args.num_letters) {
                Err(e) => {
                    println!("Input error: {}", e);
                    continue;
                }
                Ok(infos) => {
                    if let Err(e) = knowledge.add_infos(infos) {
                        println!("Bad input: {}", e);
                        continue;
                    }
                }
            }
            break;
        }

        words_file.rewind()?;
    }
}

fn print_words<T: AsRef<str>>(msg: &str, words: impl Iterator<Item=T>) {
    print!("{}: ", msg);
    let mut it = words.enumerate().peekable();
    while let Some((i, word)) = it.next() {
        print!("{}", word.as_ref());
        if i == 9 {
            break;
        }
        if it.peek().is_some() {
            print!(", ");
        }
    }
    let cnt = it.count();
    if cnt > 0 {
        println!(", and {} more", cnt);
    } else {
        println!();
    }
}

fn parse_input(inp: &str, num_letters: usize) -> Result<Vec<Info>, String> {
    let mut flag = '\0';
    let mut infos = vec![];
    for c in inp.chars() {
        if infos.len() == num_letters {
            return Err("too many letters in input".to_owned());
        }
        if c.is_whitespace() {
            continue;
        }
        if flag == '\0' {
            flag = c;
            continue;
        }
        let info = match flag {
            '*' => Info::Exact(c),
            '?' => Info::Somewhere(c),
            '!' => Info::No(c),
            _ => {
                return Err(format!("unknown annotation {:?}", flag));
            }
        };
        infos.push(info);
        flag = '\0';
    }
    if flag != '\0' {
        return Err(format!("unprocessed input {:?}", flag));
    }
    Ok(infos)
}

impl Knowledge {
    pub fn new(num_letters: usize) -> Self {
        Self {
            restrictions: vec![Restriction::Not(vec![]); num_letters],
            must_have: HashMap::new(),
        }
    }

    fn add_info(&mut self, idx: usize, info: &Info) -> Result<(), String> {
        match info {
            Info::Exact(c) => {
                if let Restriction::Exact(x) = &self.restrictions[idx] {
                    if x != c {
                        return Err(format!("you already said that letter {} is {:?}", idx, x));
                    }
                }
                self.restrictions[idx] = Restriction::Exact(*c);
            }
            Info::Somewhere(c) => {
                match &mut self.restrictions[idx] {
                    Restriction::Exact(x) => {
                        return Err(format!("you already said that letter {} is {:?}", idx, x));
                    }
                    Restriction::Not(list) => {
                        list.push(*c);
                    }
                }
                *self.must_have.entry(*c).or_insert(0) += 1;
            }
            Info::No(c) => {
                let mut add = true;
                for r in self.restrictions.iter_mut() {
                    if let Restriction::Not(list) = r {
                        if list.iter().any(|x| x == c) {
                            debug!("not adding restriction against {}; already have one somewhere", c);
                            add = false;
                            break;
                        }
                    }
                }
                if add {
                    debug!("adding restriction against {}", c);
                    for r in self.restrictions.iter_mut() {
                        if let Restriction::Not(list) = r {
                            if !list.iter().any(|x| x == c) {
                                list.push(*c);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn add_infos(&mut self, infos: Vec<Info>) -> Result<(), String> {
        let mut k2 = self.clone();
        let mut must = HashMap::new();

        for (i, info) in infos.iter().enumerate() {
            k2.add_info(i, info)?;
            match info {
                Info::Somewhere(c) | Info::Exact(c) => {
                    *must.entry(c).or_insert(0) += 1;
                }
                _ => (),
            }
        }

        for (c, num) in must.into_iter() {
            match k2.must_have.entry(*c) {
                Entry::Occupied(mut entry) => {
                    entry.insert((*entry.get()).min(num));
                }
                Entry::Vacant(entry) => {
                    entry.insert(num);
                }
            }
        }
        *self = k2;
        Ok(())
    }

    pub fn check_word(&self, word: &str) -> bool {
        if word.chars().count() != self.restrictions.len() {
            return false;
        }

        for (c, r) in word.chars().zip(self.restrictions.iter()) {
            if !('a'..='z').contains(&c) {
                return false;
            }

            let matches = match r {
                Restriction::Exact(letter) => c == *letter,
                Restriction::Not(letters) => letters.iter().all(|&l| l != c),
            };
            if !matches {
                debug!("{}: {} violates {:?}", word, c, r);
                return false;
            }
        }

        for (&c, &count) in &self.must_have {
            if word.chars().filter(|&x| x == c).count() < count {
                debug!("{}: lacks required letter {} ({} times)", word, c, count);
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_5() -> Result<(), String> {
        use Info::*;
        let mut k = Knowledge::new(5);
        k.add_infos(vec![
            No('a'),
            No('d'),
            No('i'),
            No('e'),
            No('u'),
        ])?;
        assert!(k.check_word("thorn"));
        k.add_infos(vec![
            Somewhere('t'),
            No('h'),
            Somewhere('o'),
            Somewhere('r'),
            No('n'),
        ])?;
        assert!(k.check_word("sorts"));
        k.add_infos(vec![
            No('s'),
            Exact('o'),
            Somewhere('r'),
            Somewhere('t'),
            No('s'),
        ])?;
        assert!(!k.check_word("palmy"));
        k.add_infos(vec![
            No('p'),
            No('a'),
            No('l'),
            No('m'),
            No('y'),
        ])?;
        eprintln!("{:#?}", k);
        assert!(k.check_word("robot"));
        assert!(!k.check_word("motor"));
        Ok(())
    }

    #[test]
    fn test_11_1() -> Result<(), String> {
        use Info::*;
        let mut k = Knowledge::new(11);
        // !u?l*c?e?r?a!t!i*o!n!s
        k.add_infos(vec![
            No('u'),
            Somewhere('l'),
            Exact('c'),
            Somewhere('e'),
            Somewhere('r'),
            Somewhere('a'),
            No('t'),
            No('i'),
            Exact('o'),
            No('n'),
            No('s'),
        ])?;
        assert!(k.check_word("archaeology"));
        Ok(())
    }

    #[test]
    fn test_parse() {
        use Info::*;
        assert_eq!(parse_input("!u?l*c?e?r?a!t!i*o!n!s", 11),
            Ok(vec![
                No('u'),
                Somewhere('l'),
                Exact('c'),
                Somewhere('e'),
                Somewhere('r'),
                Somewhere('a'),
                No('t'),
                No('i'),
                Exact('o'),
                No('n'),
                No('s'),
            ]));
    }

    #[test]
    fn test_11_2() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        k.add_infos(parse_input("?u!l*c!e?r!a!t?i*o?n*s", 11)?)?;
        assert!(k.check_word("incongruous"));
        Ok(())
    }

    #[test]
    fn test_11_3() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?)?;
        assert!(k.check_word("symptomatic"));
        assert!(k.check_word("masochistic"));
        k.add_infos(parse_input("?s!y?m!p!t?o!m?a*t*i*c", 11)?)?;
        assert!(!k.check_word("symptomatic"));
        assert!(k.check_word("masochistic"));
        Ok(())
    }

    #[test]
    fn test_11_4() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?)?;
        assert!(k.check_word("symptomatic"));
        assert!(k.check_word("masochistic"));
        //k.add_infos(parse_input("?s!y?m!p!t?o!m?a*t*i*c", 11)?)?;
        k.add_infos(parse_input("?m?a?s?o!c!h!i!s*t*i*c", 11)?)?;
        assert!(k.check_word("symptomatic"));
        assert!(!k.check_word("masochistic"));
        Ok(())
    }
}
