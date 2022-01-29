use std::cmp::Ordering;
use std::collections::hash_map::*;

/// Represents one letter tile.
#[derive(Debug, Clone, PartialEq)]
pub enum Info {
    /// Green letters
    Exact(char),

    /// Yellow letters
    Somewhere(char),

    /// Gray letters
    No(char),
}

/// Represents everything known about the game state.
#[derive(Debug, Clone)]
pub struct Knowledge {
    /// Restrictions on which letters can go in which spaces.
    restrictions: Vec<Restriction>,

    /// Letters that must appear *somewhere* in the word (and how many times).
    must_have: HashMap<char, usize>,
}

/// A restriction on a letter at a particular position.
#[derive(Debug, Clone)]
pub enum Restriction {
    /// Letter must be exactly the given letter.
    Exact(char),

    /// Letter must not be any of the given letters.
    Not(Vec<char>),
}

pub fn best_candidates<I, W>(
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
        if by_letters_ref.is_empty() {
            break; // shouldn't happen unless the word is not in the dictionary somehow
        }
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

pub fn check_guess(word: &str, guess: &str) -> Vec<Info> {
    let mut infos = vec![];
    for (gc, wc) in guess.chars().zip(word.chars()) {
        let info = if wc == gc {
            Info::Exact(gc)
        } else if word.contains(gc) {
            // How many are in the actual word?
            let count = word.chars()
                .filter(|&c| c == gc)
                .count();
            // How many are in the right position? These get green tiles first.
            let matched = word.chars()
                .zip(guess.chars())
                .filter(|(w, g)| w == g && *w == gc)
                .count();
            // How many yellow tiles have we assigned elsewhere?
            let elsewhere = infos.iter()
                .filter(|i| matches!(i, Info::Somewhere(c) if *c == gc))
                .count();
            if count > matched + elsewhere {
                // There's more to be found; give a yellow tile.
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
    infos
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
                for r in &mut self.restrictions {
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
                    for r in &mut self.restrictions {
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

    pub fn add_infos(&mut self, infos: &[Info], verbose: bool) -> Result<(), String> {
        let mut k2 = self.clone();
        let mut must = HashMap::new();

        for (i, info) in infos.iter().enumerate() {
            k2.add_info(i, info, verbose)?;
            match info {
                Info::Somewhere(c) | Info::Exact(c) => {
                    *must.entry(c).or_insert(0) += 1;
                }
                Info::No(_) => (),
            }
        }

        for (c, num) in must {
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

        if verbose {
            eprintln!("{}: matches", word);
        }
        true
    }
}

#[derive(PartialEq, PartialOrd)]
struct NonNan(f64);

impl TryFrom<f64> for NonNan {
    type Error = f64;
    fn try_from(f: f64) -> Result<Self, Self::Error> {
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
