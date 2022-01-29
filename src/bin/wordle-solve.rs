use std::collections::hash_map::*;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use structopt::StructOpt;
use wordle_solve::*;

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

    /// Try to guess a specific word.
    #[structopt(long)]
    word: Option<String>,

    /// Try to guess every word in the dictionary.
    ///
    /// For each word prints one line of the following format:
    ///
    /// <guesses required> <the word> (<size of dictionary>) [<guessed word> (<words remaining>)]...
    #[structopt(long)]
    check_all_words: bool,
}

fn main() -> io::Result<()> {
    let args = Args::from_args();

    let mut knowledge = Knowledge::new(args.num_letters);

    let words_file = match File::open(&args.dictionary_path) {
        Ok(f) => f,
        Err(e) => {
            println!("dictionary file {:?} could not be opened: {}", args.dictionary_path, e);
            println!("to use a different file, specify it in command line arguments");
            Args::clap().print_help().unwrap();
            println!();
            std::process::exit(1);
        }
    };

    // Build a list of all words of the correct length. Use a BTreeSet because we want the words to
    // be in order (makes it easier to debug things when order is deterministic).
    let mut dictionary = BTreeSet::<String>::new();
    for res in BufReader::new(words_file).lines() {
        let word = res?;
        if knowledge.check_word(&word, false) {
            dictionary.insert(word);
        }
    }

    // Build a map of letters to how often they occur in N-letter words.
    let mut letter_freq = HashMap::<char, f64>::new();
    for word in &dictionary {
        for c in word.chars() {
            *letter_freq.entry(c).or_insert(0.) += 1.;
        }
    }

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

    if let Some(word) = args.word {
        if word.len() != args.num_letters {
            println!("wrong number of letters in \"{}\"", word);
            std::process::exit(1);
        }
        println!("{} words in dictionary", dictionary.len());
        println!("checking: {}", word);
        let guesses = guess_word(&word, dictionary, &letter_freq);
        for (guess_num, (guess, remaining)) in guesses.iter().enumerate() {
            if guess.is_empty() {
                println!("dunno lol");
                println!("is the word in the dictionary?");
                break;
            }
            println!("  {}: guessing {}", guess_num, guess);
            println!("    {} candidates left", remaining);
        }
        println!("{} guesses required", guesses.len());
        return Ok(());
    }

    if args.check_all_words {
        check_all_words(&dictionary, &letter_freq);
        return Ok(());
    }

    loop {
        if dictionary.is_empty() {
            println!("no candidates left!");
            return Ok(());
        }

        println!("{} candidates.", dictionary.len());
        let best = best_candidates(dictionary.iter(), &knowledge, &letter_freq);
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
                    if let Err(e) = knowledge.add_infos(&infos, args.verbose) {
                        println!("Bad input: {}", e);
                        continue;
                    }
                }
            }
            break;
        }

        dictionary.retain(|word| knowledge.check_word(word, args.verbose));
    }
}

fn check_all_words(dictionary: &BTreeSet<String>, letter_freq: &HashMap<char, f64>) {
    for word in dictionary {
        let guesses = guess_word(word, dictionary.clone(), letter_freq);
        print!("{} {} ({})", guesses.len(), word, dictionary.len());
        for (guess, remaining) in guesses {
            print!(" {} ({})", guess, remaining);
        }
        println!();
    }
}

fn guess_word(
    word: &str,
    mut candidates: BTreeSet<String>,
    letter_freq: &HashMap<char, f64>,
) -> Vec<(String, usize)> {
    let mut guesses = vec![];
    let mut knowledge = Knowledge::new(word.len());

    loop {
        let best_guesses = best_candidates(candidates.iter(), &knowledge, letter_freq);
        if best_guesses.is_empty() {
            guesses.push((String::new(), 0));
            return guesses;
        }
        let guess = best_guesses[0].clone();
        if guess == word {
            guesses.push((guess, 1));
            return guesses;
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

        if let Err(e) = knowledge.add_infos(&infos, false) {
            panic!("ERROR on {} (guessing {}): {}", word, guess, e);
        }

        candidates.retain(|word| knowledge.check_word(word, false));
        guesses.push((guess, candidates.len()));
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_5() -> Result<(), String> {
        use Info::*;
        let mut k = Knowledge::new(5);
        k.add_infos(&[
            No('a'),
            No('d'),
            No('i'),
            No('e'),
            No('u'),
        ], true)?;
        assert!(k.check_word("thorn", true));
        k.add_infos(&[
            Somewhere('t'),
            No('h'),
            Somewhere('o'),
            Somewhere('r'),
            No('n'),
        ], true)?;
        assert!(k.check_word("sorts", true));
        k.add_infos(&[
            No('s'),
            Exact('o'),
            Somewhere('r'),
            Somewhere('t'),
            No('s'),
        ], true)?;
        assert!(!k.check_word("palmy", true));
        k.add_infos(&[
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
        k.add_infos(&[
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
        k.add_infos(&parse_input("?u!l*c!e?r!a!t?i*o?n*s", 11)?, true)?;
        assert!(k.check_word("incongruous", true));
        Ok(())
    }

    #[test]
    fn test_11_3() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(&parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        k.add_infos(&parse_input("?s!y?m!p!t?o!m?a*t*i*c", 11)?, true)?;
        assert!(!k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        Ok(())
    }

    #[test]
    fn test_11_4() -> Result<(), String> {
        let mut k = Knowledge::new(11);
        // symptomatic / masochistic
        k.add_infos(&parse_input("!u!l?c!e!r?a?t?i?o!n?s", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(k.check_word("masochistic", true));
        k.add_infos(&parse_input("?m?a?s?o!c!h!i!s*t*i*c", 11)?, true)?;
        assert!(k.check_word("symptomatic", true));
        assert!(!k.check_word("masochistic", true));
        Ok(())
    }
}
