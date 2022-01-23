use std::cmp::Ordering;
use std::collections::hash_map::*;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, Write};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Args {
    /// How many letters in the word?
    #[structopt(default_value = "5")]
    num_letters: usize,

    /// Path to a dictionary file, with one word per line.
    #[structopt(default_value = "/usr/share/dict/words")]
    dictionary_path: String,

    /// Enable debug output?
    #[structopt(short = "v", long)]
    verbose: bool,

    /// Search for the words which require the most guesses.
    #[structopt(long)]
    most_difficult: bool,

    /// Try to guess a specific word.
    #[structopt(long)]
    word: String,
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
    let args = Args::from_args();

    let mut knowledge = Knowledge::new(args.num_letters);

    let mut words_file = match File::open(&args.dictionary_path) {
        Ok(f) => f,
        Err(e) => {
            println!("dictionary file {:?} could not be opened: {}", args.dictionary_path, e);
            println!("to use a different file, specify it in command line arguments");
            Args::clap().print_help().unwrap();
            println!();
            std::process::exit(1);
        }
    };

    // Build a map of letters to how often they occur in N-letter words.
    let mut letter_freq = HashMap::<char, f64>::new();
    for res in BufReader::new(&mut words_file).lines() {
        let word = res?;
        // Knowledge is empty, so this just checks word length and letters against the alphabet.
        if !knowledge.check_word(&word, args.verbose) {
            continue;
        }
        for c in word.chars() {
            *letter_freq.entry(c).or_insert(0.) += 1.;
        }
    }
    words_file.rewind()?;

    // Normalize by total number of letters.
    let total_letters = letter_freq.iter().map(|(_c, count)| count).sum::<f64>();
    for v in letter_freq.values_mut() {
        *v /= total_letters;
    }

    if args.verbose {
        let mut letters = letter_freq.iter().map(|(c, f)| (*c, *f)).collect::<Vec<(char, f64)>>();
        letters.sort_unstable_by(|(_, f1), (_, f2)| f2.partial_cmp(f1).unwrap());
        eprintln!("letter frequency:");
        for (letter, freq) in &letters {
            eprintln!("\t('{}', {})", letter, freq);
        }
    }

    if args.most_difficult {
        let (words, guesses) = most_difficult_word(words_file, args.num_letters, &letter_freq)?;
        println!("worst word(s):");
        for word in &words {
            println!("\t{}", word);
        }
        println!("in {} guesses", guesses);
        return Ok(());
    }

    if !args.word.is_empty() {
        let all_candidates = BufReader::new(words_file)
            .lines()
            .filter_map(|res| {
                match res {
                    Ok(word) => {
                        if knowledge.check_word(&word, false) {
                            Some(Ok(word))
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            })
            .collect::<io::Result<Vec<_>>>()?;
        let n = guess_word(&args.word, &all_candidates, &letter_freq);
        println!("{} tries required", n);
        return Ok(());
    }

    loop {
        let mut candidates = vec![];
        for res in BufReader::new(&mut words_file).lines() {
            let word = res?;

            if knowledge.check_word(&word, args.verbose) {
                if args.verbose {
                    eprintln!("adding {}", word);
                }
                candidates.push(word);
            }
        }

        if candidates.is_empty() {
            println!("no candidates left!");
            return Ok(());
        }

        println!("{} candidates.", candidates.len());
        let best = best_candidates(candidates.iter(), &knowledge, &letter_freq);
        print_words("By most unique letters and letter frequency",
            best.iter().map(|w| format!("\n\t{}", w)));

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
                    if let Err(e) = knowledge.add_infos(infos, args.verbose) {
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

fn most_difficult_word(mut dict_file: File, num_letters: usize, letter_freq: &HashMap<char, f64>) -> io::Result<(Vec<String>, u64)> {
    let mut all_candidates = vec![];
    let empty_knowledge = Knowledge::new(num_letters);
    for res in BufReader::new(&mut dict_file).lines() {
        let word = res?;
        if empty_knowledge.check_word(&word, false) {
            all_candidates.push(word);
        }
    }
    dict_file.rewind()?;

    let mut worst = (vec![String::new()], 0);
    for word in &all_candidates {
        let guess_num = guess_word(word, &all_candidates, letter_freq);
        match guess_num.cmp(&worst.1) {
            Ordering::Equal => {
                println!("tie for worst: {} in {} guesses", word, guess_num);
                worst.0.push(word.to_owned());
            }
            Ordering::Greater => {
                println!("new worst: {} in {} guesses", word, guess_num);
                worst = (vec![word.to_owned()], guess_num);
            }
            _ => ()
        }
    }

    Ok(worst)
}

fn guess_word(word: &str, words: &[String], letter_freq: &HashMap<char, f64>) -> u64 {
    println!("checking {}", word);
    let mut candidates = words.to_vec();
    let mut knowledge = Knowledge::new(word.len());

    for guess_num in 1 .. {
        let best_guesses = best_candidates(candidates.iter(), &knowledge, letter_freq);
        let guess = best_guesses[0];
        println!("  guessing {}", guess);
        if guess == word {
            return guess_num;
        }

        let mut infos = vec![];
        for (gc, wc) in guess.chars().zip(word.chars()) {
            let info = if wc == gc {
                Info::Exact(gc)
            } else if word.contains(gc) {
                // How many are in the actual word?
                let count = word.chars()
                    .filter(|&c| c == gc)
                    .count();
                // How many match our guess? These get green tiles first.
                let matched = word.chars()
                    .zip(guess.chars())
                    .filter(|(w, g)| w == g && *w == gc)
                    .count();
                // How many yellow tiles have we assigned elsewhere?
                let elsewhere = infos.iter()
                    .filter(|i| matches!(i, Info::Somewhere(c) if *c == gc))
                    .count();
                if count > matched + elsewhere {
                    // There's more to be found, give a yellow tile.
                    Info::Somewhere(gc)
                } else {
                    // Enough non-gray tiles have been assigned already.
                    Info::No(gc)
                }
            } else {
                Info::No(gc)
            };
            infos.push(info);
        }

        if let Err(e) = knowledge.add_infos(infos, false) {
            eprintln!("ERROR on {}: {}", word, e);
            break;
        }

        let mut new_candidates = vec![];
        for cword in candidates.into_iter() {
            if knowledge.check_word(&cword, false) {
                new_candidates.push(cword);
            }
        }
        candidates = new_candidates;
        println!("  {} new candidates", candidates.len());
    }
    unreachable!()
}

fn best_candidates<I, W>(
    candidates: I,
    knowledge: &Knowledge,
    letter_freq: &HashMap<char, f64>,
) -> Vec<<W as ToOwned>::Owned>
    where I: Iterator<Item=W>,
          W: AsRef<str> + ToOwned,
{
    let mut by_letters = candidates
        .map(|word| {
            let mut letters = word.as_ref().chars().collect::<Vec<_>>();
            letters.sort_unstable();
            letters.dedup();
            (word, letters.len())
        })
        .collect::<Vec<_>>();
    by_letters.sort_unstable_by(|(_, c1), (_, c2)| c2.cmp(c1));

    let mut results = vec![];

    // Start with the words with the most unique letters. If that gives less than 10 results, then
    // continue ranking and adding words with fewer unique letters.
    let mut by_letters_ref = &mut by_letters[..];
    while results.len() < 10 {
        let most_letters_count = by_letters_ref[0].1;
        let len = {
            // Only look at the words with the most unique letters.
            let most_unique_letters = by_letters_ref.split_mut(|(_, count)| *count < most_letters_count).next().unwrap();
            if most_unique_letters.len() != 1 {
                // Sort the words score, according to letter frequency.
                most_unique_letters.sort_by_cached_key::<NonNan, _>(|(word, _)| {
                    word.as_ref().chars()
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
                                // Otherwise, add up the frequency of letters in the dictionary.
                                // Negative, so they are sorted with highest score first.
                                -letter_freq[&c]
                            }
                        })
                        .sum::<f64>()
                        .try_into() // into NonNan
                        .unwrap()
                });
            }
            results.extend(
                most_unique_letters
                    .iter()
                    .map(|(word, _)| word.to_owned())
            );
            most_unique_letters.len()
        };

        // Subsequent loop iterations will skip over these words and begin considering words with
        // fewer unique letters.
        by_letters_ref = &mut by_letters_ref[len .. ];
        if by_letters_ref.is_empty() {
            break;
        }
    }
    results
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
    let mut flag = None;
    let mut infos = vec![];
    for c in inp.chars() {
        if infos.len() == num_letters {
            return Err("too many letters in input".to_owned());
        }
        if c.is_whitespace() {
            continue;
        }
        if flag.is_none() {
            flag = Some(c);
            continue;
        }
        let info = match flag.unwrap() {
            '*' => Info::Exact(c),
            '?' => Info::Somewhere(c),
            '!' => Info::No(c),
            other => {
                return Err(format!("unknown annotation {:?}", other));
            }
        };
        infos.push(info);
        flag = None;
    }
    if let Some(extra) = flag {
        return Err(format!("unprocessed input {:?}", extra));
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

    fn add_info(&mut self, idx: usize, info: &Info, verbose: bool) -> Result<(), String> {
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
                            if verbose {
                                eprintln!("not adding restriction against {}; already have one somewhere", c);
                            }
                            add = false;
                            break;
                        }
                    }
                }
                if add {
                    if verbose {
                        eprintln!("adding restriction against {}", c);
                    }
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

    pub fn add_infos(&mut self, infos: Vec<Info>, verbose: bool) -> Result<(), String> {
        let mut k2 = self.clone();
        let mut must = HashMap::new();

        for (i, info) in infos.iter().enumerate() {
            k2.add_info(i, info, verbose)?;
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

    pub fn check_word(&self, word: &str, verbose: bool) -> bool {
        if word.chars().count() != self.restrictions.len() {
            return false;
        }

        for (i, (c, r)) in word.chars().zip(self.restrictions.iter()).enumerate() {
            if !('a'..='z').contains(&c) {
                return false;
            }

            let matches = match r {
                Restriction::Exact(letter) => c == *letter,
                Restriction::Not(letters) => letters.iter().all(|&l| l != c),
            };
            if !matches {
                if verbose {
                    eprintln!("{}: {} violates {:?} at {}", word, c, r, i);
                }
                return false;
            }
        }

        for (&c, &count) in &self.must_have {
            if word.chars().filter(|&x| x == c).count() < count {
                if verbose {
                    eprintln!("{}: lacks required letter {} ({} times)", word, c, count);
                }
                return false;
            }
        }

        true
    }
}

#[derive(PartialEq, PartialOrd)]
struct NonNan(f64);

impl TryFrom<f64> for NonNan {
    type Error = f64;
    fn try_from(f: f64) -> Result<Self, f64> {
        if f.is_nan() {
            Err(f)
        } else {
            Ok(Self(f))
        }
    }
}

#[allow(clippy::derive_ord_xor_partial_ord)] // Ord just calls PartialOrd
impl std::cmp::Ord for NonNan {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl std::cmp::Eq for NonNan {}

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
        ], true)?;
        assert!(k.check_word("thorn", true));
        k.add_infos(vec![
            Somewhere('t'),
            No('h'),
            Somewhere('o'),
            Somewhere('r'),
            No('n'),
        ], true)?;
        assert!(k.check_word("sorts", true));
        k.add_infos(vec![
            No('s'),
            Exact('o'),
            Somewhere('r'),
            Somewhere('t'),
            No('s'),
        ], true)?;
        assert!(!k.check_word("palmy", true));
        k.add_infos(vec![
            No('p'),
            No('a'),
            No('l'),
            No('m'),
            No('y'),
        ], true)?;
        eprintln!("{:#?}", k);
        assert!(k.check_word("robot", true));
        assert!(!k.check_word("motor", true));
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
        ], true)?;
        assert!(k.check_word("archaeology", true));
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
        k.add_infos(parse_input("?u!l*c!e?r!a!t?i*o?n*s", 11)?, true)?;
        assert!(k.check_word("incongruous", true));
        Ok(())
    }

    #[test]
    fn test_11_3() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        k.add_infos(parse_input("?s!y?m!p!t?o!m?a*t*i*c", 11)?, true)?;
        assert!(!k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        Ok(())
    }

    #[test]
    fn test_11_4() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        //k.add_infos(parse_input("?s!y?m!p!t?o!m?a*t*i*c", 11)?)?;
        k.add_infos(parse_input("?m?a?s?o!c!h!i!s*t*i*c", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(!k.check_word("masochistic", true));
        Ok(())
    }
}
