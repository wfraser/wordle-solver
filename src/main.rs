use log::{debug, error};
use std::collections::{HashSet, HashMap};
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

#[derive(Debug, Clone)]
enum Restriction {
    Exact(char),
    Not(Vec<char>),
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args = Args::from_args();

    let mut restrictions = vec![Restriction::Not(vec![]); args.num_letters];
    let mut must = HashMap::<char, usize>::new();

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
        'word: for res in BufReader::new(&mut words_file).lines() {
            let word = res?;

            if word.chars().count() != args.num_letters {
                continue 'word;
            }

            for (c, r) in word.chars().zip(restrictions.iter()) {
                if !('a'..='z').contains(&c) {
                    continue;
                }

                let matches = match r {
                    Restriction::Exact(letter) => c == *letter,
                    Restriction::Not(letters) => letters.iter().all(|&l| l != c),
                };
                if !matches {
                    debug!("{}: {} violates {:?}", word, c, r);
                    continue 'word;
                }
            }

            for (&c, &count) in &must {
                if word.chars().filter(|&x| x == c).count() < count {
                    debug!("{}: lacks required letter {} ({} times)", word, c, count);
                    continue 'word;
                }
            }
            
            debug!("adding {}", word);
            candidates.push(word);
        }

        if candidates.is_empty() {
            println!("no candidates left!");
            return Ok(());
        }

        print_words("candidates", candidates.iter());

        let mut by_letters = candidates
            .iter()
            .map(|word| {
                (word.clone(), HashSet::<char>::from_iter(word.chars()).len())
            })
            .collect::<Vec<_>>();
        by_letters.sort_unstable_by(|(_word1, count1), (_word2, count2)| count2.cmp(count1));

        let most_unique_letters = by_letters.split(|(_word, count)| *count < by_letters[0].1).next().unwrap();

        if most_unique_letters.len() > 1 {
            let mut by_freq = most_unique_letters
                .into_iter()
                .map(|(word, _count)| {
                    (word, word.chars()
                        .map(|c| LETTER_FREQ.iter().find(|(l, _f)| *l == c).unwrap().1)
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
            print!("Type the guess you made. Annotate each letter with: green=*, yellow=?, gray=!: ");
            io::stdout().flush()?;
            let mut inp = String::new();
            io::stdin().read_line(&mut inp)?;
            inp = inp.trim().to_owned();
            if inp.is_empty() {
                return Ok(());
            }
            if let Err(e) = parse_input(&inp, args.num_letters, &mut restrictions, &mut must) {
                println!("Input error: {}", e);
                continue;
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

fn parse_input(inp: &str, num_letters: usize, restrictions: &mut Vec<Restriction>, must: &mut HashMap<char, usize>) -> Result<(), String> {
    let mut flag = '\0';
    let mut idx = 0;
    for c in inp.chars() {
        if idx >= num_letters {
            return Err("too many letters in input".to_owned());
        }
        if c.is_whitespace() {
            continue;
        }
        if flag == '\0' {
            flag = c;
            continue;
        }
        match flag {
            '*' => {
                if let Restriction::Exact(x) = restrictions[idx] {
                    if x != c {
                        return Err(format!("you already said that letter {} is {:?}", idx, x));
                    }
                }
                restrictions[idx] = Restriction::Exact(c);
                *must.entry(c).or_insert(0) += 1;
            }
            '?' => {
                match &mut restrictions[idx] {
                    Restriction::Exact(x) => {
                        return Err(format!("you already said that letter {} is {:?}", idx, x));
                    }
                    Restriction::Not(list) => {
                        list.push(c);
                    }
                }
                *must.entry(c).or_insert(0) += 1;
            }
            '!' => {
                let mut add = true;
                for r in restrictions.iter_mut() {
                    match r {
                        Restriction::Not(list) => {
                            if list.iter().find(|&&x| x == c).is_some() {
                                debug!("not adding restriction against {}; already have one somewhere", c);
                                add = false;
                                break;
                            }
                        }
                        /*Restriction::Exact(x) if x == c => {
                            debug!("not adding restriction against {}; already required somewhere");
                            add = false;
                            break;
                        }*/
                        _ => (),
                    }
                }
                if add {
                    debug!("adding restriction against {}", c);
                    for r in restrictions.iter_mut() {
                        if let Restriction::Not(list) = r {
                            if list.iter().find(|&&x| x == c).is_none() {
                                list.push(c);
                            }
                        }
                    }
                }
            }
            _ => {
                return Err(format!("unknown annotation {:?}", flag));
            }
        }
        idx += 1;
        flag = '\0';
    }
    if flag != '\0' {
        return Err(format!("unprocessed input {:?}", flag));
    }
    Ok(())
}
