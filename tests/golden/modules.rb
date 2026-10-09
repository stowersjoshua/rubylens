# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# use std::collections::{HashMap, HashSet as Set}
# use std::io::{self, Read}

Index = HashMap  # type alias: HashMap<String, Vec<usize>>

MAX_ITEMS = 100  # usize

GREETING = "hello"  # static &str

module Storage
  # fn save(key: &str) -> bool
  def self.save(key)
    !key.empty?
  end

  module Disk
    # fn path() -> &'static str
    def self.path
      "/tmp"
    end
  end
end

# fn run() -> bool
def run
  _ = Storage::Disk.path()
  Storage.save("k") && Crate::MAX_ITEMS > 0
end
