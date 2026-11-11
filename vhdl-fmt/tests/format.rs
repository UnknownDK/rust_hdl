// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use vhdl_fmt::{format, FormatError, KeywordCase, Settings};

fn formatted_with(input: &[u8], settings: &Settings) -> Vec<u8> {
    let output = format(input, settings).expect("input formats");
    assert_eq!(
        format(&output, settings).expect("output formats"),
        output,
        "formatting is idempotent"
    );
    output
}

fn check(input: &str, expected: &str) {
    let output = formatted_with(input.as_bytes(), &Settings::default());
    assert_eq!(String::from_utf8(output).unwrap(), expected);
}

#[test]
fn spacing_inside_a_line_is_normalized() {
    check(
        "entity e is port(a:in bit:=-1;b:out t(-1 to x**2)); end;\n",
        "entity e is\n    port (a: in bit := -1; b: out t(-1 to x ** 2));\nend;\n",
    );
    check(
        "package p is alias x is y[integer,bit return boolean]; \
         signal s:integer range -1 to 2 :=-(3)+abs(-2); \
         type arr is array(natural range<>)of bit; end;\n",
        "package p is\n    alias x is y[integer, bit return boolean];\n    \
         signal s: integer range -1 to 2 := -(3) + abs (-2);\n    \
         type arr is array (natural range <>) of bit;\nend;\n",
    );
    check(
        "architecture a of e is begin x<=not(a)and(b); \
         y<=t'(1,2)&x'range(1)&f(1)(2).g; \
         z<=a after 2 ns,b after 3 ns; assert(x)report\"a\"severity note; end;\n",
        "architecture a of e is\nbegin\n    x <= not (a) and (b);\n    \
         y <= t'(1, 2) & x'range(1) & f(1)(2).g;\n    \
         z <= a after 2 ns, b after 3 ns;\n    assert (x)\n        report \"a\"\n        severity note;\nend;\n",
    );
}

#[test]
fn declarations_and_statements_get_a_line_and_a_level_each() {
    check(
        "library ieee; use ieee.std_logic_1164.all; library foo;\n\
         entity e is generic (W : natural); port (a : in bit); end entity;\n\
         architecture a of e is signal s : bit; \
         function f(a : integer) return integer is begin return a + 1; end function; begin\n\
         p : process(clk) variable v : integer; begin \
         if rising_edge(clk) then yy <= 0; elsif a then null; else yy <= 1; end if; \
         case s is when a | b => null; when others => x := 1; end case; \
         for i in 0 to 3 loop wait; end loop; end process;\n\
         u : entity work.foo generic map (W => 8) port map (clk => clk);\n\
         g : for i in 0 to 3 generate signal s : bit; begin y(i) <= z(i); end generate;\n\
         end architecture;\n",
        "\
library ieee;
    use ieee.std_logic_1164.all;

library foo;

entity e is
    generic (W: natural);
    port (a: in bit);
end entity;

architecture a of e is
    signal s: bit;

    function f(a: integer) return integer is
    begin
        return a + 1;
    end function;
begin
    p: process(clk)
        variable v: integer;
    begin
        if rising_edge(clk) then
            yy <= 0;
        elsif a then
            null;
        else
            yy <= 1;
        end if;
        case s is
            when a | b =>
                null;
            when others =>
                x := 1;
        end case;
        for i in 0 to 3 loop
            wait;
        end loop;
    end process;

    u: entity work.foo
        generic map (W => 8)
        port map (clk => clk);
    g: for i in 0 to 3 generate
        signal s: bit;
    begin
        y(i) <= z(i);
    end generate;
end architecture;
",
    );
}

#[test]
fn types_and_configurations_are_structured() {
    check(
        "package p is type r is record a : bit; b : bit; end record; \
         type t is range 0 to 10 units fs; ps = 1000 fs; end units; \
         type prot is protected procedure x; end protected; \
         component c is generic (g : bit); port (p : bit); end component; end package;\n\
         configuration cfg of e is for a for u : comp use entity work.foo(rtl); end for; end for; \
         end configuration;\n",
        "\
package p is
    type r is record
        a: bit;
        b: bit;
    end record;
    type t is range 0 to 10
        units
            fs;
            ps = 1000 fs;
        end units;
    type prot is protected
        procedure x;
    end protected;
    component c is
        generic (g: bit);
        port (p: bit);
    end component;
end package;

configuration cfg of e is
    for a
        for u: comp
            use entity work.foo(rtl);
        end for;
    end for;
end configuration;
",
    );
}

#[test]
fn line_breaks_of_the_source_are_not_kept() {
    check(
        "architecture a of e is\nbegin\n  x <= a when sel = '1'\n         else b when sel = '0'\n           \
         and en = '1'\n         else c;\n\n\n\n  y <=\n    d;\nend;\n",
        "\
architecture a of e is
begin
    x <= a when sel = '1' else b when sel = '0' and en = '1' else c;

    y <= d;
end;
",
    );
    check(
        "\n\nentity e is   \n\tport (\n      a : in bit\n  );\n\n\n\nend entity;\n\n\n",
        "entity e is\n    port (a: in bit);\n\nend entity;\n",
    );
}

#[test]
fn statements_wrap_when_they_do_not_fit() {
    let mut settings = Settings::default();
    settings.format.max_width = 50;
    let output = formatted_with(
        b"architecture a of e is begin \
          x <= a when sel = \"00\" else b after 1 ns when c else d_long_name; \
          with sel select y <= a when \"00\", b when \"01\" | \"10\", c_long when others; \
          a_rather_long_target_signal_name_here <= another_long_signal_name; \
          p : process begin \
          if first_condition_signal = '1' and second_condition_signal = '1' then null; end if; \
          y <= transport a_long_name after 1 ns, b_long_name after 2 ns; \
          wait on first_signal, second_signal until condition for 1 ns; \
          report \"a message that is long\" & integer'image(x) severity failure; \
          return some_function_name(argument_one) + offset_value; \
          end process; end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "\
architecture a of e is
begin
    x <= a when sel = \"00\"
        else b after 1 ns when c
        else d_long_name;
    with sel select
        y <= a when \"00\",
            b when \"01\" | \"10\",
            c_long when others;
    a_rather_long_target_signal_name_here <=
        another_long_signal_name;

    p: process
    begin
        if first_condition_signal = '1'
            and second_condition_signal = '1'
        then
            null;
        end if;
        y <= transport a_long_name after 1 ns,
            b_long_name after 2 ns;
        wait
            on first_signal, second_signal
            until condition
            for 1 ns;
        report
            \"a message that is long\"
                & integer'image(x)
            severity failure;
        return some_function_name(argument_one)
            + offset_value;
    end process;
end;
"
    );
}

#[test]
fn declarations_wrap_when_they_do_not_fit() {
    let mut settings = Settings::default();
    settings.format.max_width = 60;
    let output = formatted_with(
        b"package p is \
          type memory_schedule_vector is array (natural range <>) of schedule_data_type; \
          constant a_rather_long_constant_name : a_long_type_name := a_long_default_value; \
          signal first_signal_name, second_signal_name, third_signal_name : bit; \
          function resolve(a : integer; b : integer) return a_long_result_type_name; \
          attribute foreign of create_it : function is \"VHPI systf; create\"; \
          end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "\
package p is
    type memory_schedule_vector is array (
        natural range <>
    ) of schedule_data_type;
    constant a_rather_long_constant_name:
        a_long_type_name := a_long_default_value;
    signal first_signal_name,
        second_signal_name,
        third_signal_name: bit;
    function resolve(a: integer; b: integer)
        return a_long_result_type_name;
    attribute foreign of
        create_it: function is
        \"VHPI systf; create\";
end;
"
    );
}

#[test]
fn lists_stay_on_one_line_up_to_the_argument_limit() {
    check(
        "entity e is generic (W : natural := 8;\n D : natural); \
         port (clk, rst : in std_logic; q : out std_logic_vector(W-1 downto 0)); end;\n",
        "\
entity e is
    generic (W: natural := 8; D: natural);
    port (
        clk, rst: in std_logic;
        q: out std_logic_vector(W - 1 downto 0)
    );
end;
",
    );
    check(
        "architecture a of e is constant m : rec := (a => 1, b => 2, c => 3); begin \
         u : entity work.foo generic map (W => 8) port map (clk => clk, rst => rst, -- reset\n\
         d => x(3 downto 0) & x(7 downto 4), q => open); \
         x <= f(a,\n b) when s else g(a, b, c); end;\n",
        "\
architecture a of e is
    constant m: rec := (
        a => 1,
        b => 2,
        c => 3
    );
begin
    u: entity work.foo
        generic map (W => 8)
        port map (
            clk => clk,
            rst => rst, -- reset
            d => x(3 downto 0) & x(7 downto 4),
            q => open
        );
    x <= f(a, b) when s else g(a, b, c);
end;
",
    );
}

#[test]
fn lists_wrap_when_they_do_not_fit() {
    check(
        "package p is type state is (state_idle, state_running_first, state_running_second, \
         state_running_third, state_done_and_waiting); type t is (idle, run);\n\
         constant c : t := (FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, \
         FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, -1, FALSE); constant d : t := (1,\n 2);\n\
         procedure x(Name1, Name2, Name3, Name4, Name5, Name6, Name7, Name8, Name9, Name10, Name11, \
         Name12, Name13 : string); end;\n",
        "\
package p is
    type state is (
        state_idle,
        state_running_first,
        state_running_second,
        state_running_third,
        state_done_and_waiting
    );
    type t is (idle, run);
    constant c: t := (
        FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE,
        FALSE, FALSE, FALSE, -1, FALSE
    );
    constant d: t := (1, 2);
    procedure x(
        Name1,
        Name2,
        Name3,
        Name4,
        Name5,
        Name6,
        Name7,
        Name8,
        Name9,
        Name10,
        Name11,
        Name12,
        Name13: string
    );
end;
",
    );
}

#[test]
fn expressions_in_lists_break_before_operators() {
    check(
        "architecture a of e is begin p : process begin \
         report_it(\"message\", a_first_very_long_argument_name + another_long_name + \
         yet_another_long_name * factor + and_one_more_name - final_long_name); end process; end;\n",
        "\
architecture a of e is
begin
    p: process
    begin
        report_it(
            \"message\",
            a_first_very_long_argument_name
                + another_long_name
                + yet_another_long_name * factor
                + and_one_more_name
                - final_long_name
        );
    end process;
end;
",
    );
}

#[test]
fn comments_inside_lists_keep_their_place() {
    check(
        "entity e is port (\n  -- clocks\n  clk : in bit; -- the clock\n  \
         /* data */ d : in bit\n  -- no more\n); end;\n\
         package p is alias s is <<signal .tb.x:bit>>; end;\n",
        "\
entity e is
    port (
        -- clocks
        clk: in bit; -- the clock
        /* data */ d: in bit
        -- no more
    );
end;

package p is
    alias s is << signal .tb.x : bit >>;
end;
",
    );
}

#[test]
fn the_argument_limit_is_configurable() {
    let mut settings = Settings::default();
    settings.format.inline_argument_limit = 0;
    settings.format.indent_width = 2;
    settings.format.max_width = 30;
    let output = formatted_with(
        b"package p is constant c : t := f(a) + (1, 2); constant long_name : t := (others => zero); end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "\
package p is
  constant c: t := f(a)
    + (1, 2);
  constant long_name: t := (
    others => zero
  );
end;
"
    );
}

#[test]
fn comments_stay_with_their_code() {
    check(
        "-- header   \nentity e is -- trailing\nend /* inline */ entity; -- last",
        "-- header\nentity e is -- trailing\nend /* inline */ entity; -- last\n",
    );
    check("-- only a comment", "-- only a comment\n");
    check(
        "architecture a of e is\nbegin\n  p : process\n  begin\n-- at column zero\n    x <= a;  -- set\n    \
         -- closes the branch\n  end process;\n  /* block */ y <= 1; /* same line */ z <= 2;\nend;\n",
        "\
architecture a of e is
begin
    p: process
    begin
        -- at column zero
        x <= a; -- set
        -- closes the branch
    end process;

    /* block */ y <= 1; /* same line */
    z <= 2;
end;
",
    );
}

#[test]
fn empty_input_stays_empty() {
    check("", "");
    check("  \n\n", "");
}

#[test]
fn keyword_case_leaves_identifiers_and_literals_alone() {
    let mut settings = Settings::default();
    settings.format.keyword_case = KeywordCase::Upper;
    let output = formatted_with(
        b"entity Foo is end entity; -- entity\narchitecture a of Foo is constant c: String := \"is\"; begin end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "ENTITY Foo IS\nEND ENTITY; -- entity\n\nARCHITECTURE a OF Foo IS\n    CONSTANT c: String := \"is\";\nBEGIN\nEND;\n"
    );
    check(
        "ENTITY Foo IS END ENTITY;\n",
        "entity Foo is\nend entity;\n",
    );
}

#[test]
fn disabled_regions_are_verbatim() {
    check(
        "package   p is\n  -- fmt: off\n  constant   m : t :=  (1,0,\n                        0,1);\n  -- fmt: on\n  constant   n : t:=1;\nend;\n",
        "package p is\n    -- fmt: off\n  constant   m : t :=  (1,0,\n                        0,1);\n  -- fmt: on\n    constant n: t := 1;\nend;\n",
    );
    // An unterminated region runs to the end of the file.
    check(
        "package   p is\n-- fmt: off\nconstant   n : t:=1;\nend  ;\n\n",
        "package p is\n    -- fmt: off\nconstant   n : t:=1;\nend  ;\n",
    );
}

#[test]
fn line_endings_follow_the_input() {
    check(
        "entity   e is\r\nend entity;\r\n",
        "entity e is\r\nend entity;\r\n",
    );
    check(
        "entity   e is end entity;\r\n",
        "entity e is\r\nend entity;\r\n",
    );
}

#[test]
fn bytes_outside_ascii_survive_in_any_encoding() {
    // Latin-1 in a string and a comment.
    let latin1 = b"entity   e is constant s : string := \"\xe9\"; end; -- \xe9\n";
    assert_eq!(
        formatted_with(latin1, &Settings::default()),
        b"entity e is\n    constant s: string := \"\xe9\";\nend; -- \xe9\n"
    );
    check(
        "entity   e is end; -- naïve ☃\n",
        "entity e is\nend; -- naïve ☃\n",
    );
}

#[test]
fn syntax_errors_are_located_and_nothing_is_formatted() {
    let error = format(
        b"entity e is\n  port (a : in bit;);\nend;\n",
        &Settings::default(),
    )
    .expect_err("a trailing semicolon is not VHDL-2008");
    let FormatError::Syntax(errors) = error else {
        panic!("expected a syntax error, got {error:?}");
    };
    assert_eq!(
        (errors[0].location.line, errors[0].location.column),
        (2, 20)
    );
}

#[test]
fn columns_are_aligned_on_request() {
    let mut settings = Settings::default();
    settings.format.align_declarations = true;
    settings.format.align_associations = true;
    settings.format.align_assignments = true;
    let output = formatted_with(
        b"entity e is port (clk : in bit; reset_n : in bit;\n\n data_out : out bit := '0'; q : out bit); end;\n\
          architecture a of e is signal state : t := idle; signal next_state_value : t;\n\
          -- payload\n signal p : bit; begin\n\
          state <= idle; payload_valid <= '0'; lbl : x <= y;\n\
          u : entity work.foo port map (clk => clk, reset_n => reset_n, q => open);\n\
          v : entity work.foo port map (a => b, long_name => c); end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "\
entity e is
    port (
        clk     : in bit;
        reset_n : in bit;

        data_out : out bit := '0';
        q        : out bit
    );
end;

architecture a of e is
    signal state            : t := idle;
    signal next_state_value : t;
    -- payload
    signal p : bit;
begin
    state         <= idle;
    payload_valid <= '0';
    lbl: x <= y;
    u: entity work.foo
        port map (
            clk     => clk,
            reset_n => reset_n,
            q       => open
        );
    v: entity work.foo
        port map (a => b, long_name => c);
end;
"
    );
}

#[test]
fn rows_that_do_not_fit_on_a_line_are_not_aligned() {
    let mut settings = Settings::default();
    settings.format.align_assignments = true;
    settings.format.max_width = 40;
    let output = formatted_with(
        b"architecture a of e is begin a <= b; long_target <= c; \
          x <= a_very_long_expression_name + another_long_name; yy <= d; end;\n",
        &settings,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "\
architecture a of e is
begin
    a           <= b;
    long_target <= c;
    x <= a_very_long_expression_name
        + another_long_name;
    yy <= d;
end;
"
    );
}

#[test]
fn one_declaration_or_statement_can_be_skipped() {
    check(
        "architecture a of e is\n  -- fmt: skip\n  constant   m : t :=  (1,0,\n                        0,1);\n  \
         signal   s:bit;\nbegin\n  x   <=    1; -- fmt: skip\n  y   <=    2;\nend;\n",
        "\
architecture a of e is
    -- fmt: skip
    constant   m : t :=  (1,0,
                        0,1);
    signal s: bit;
begin
    x   <=    1; -- fmt: skip
    y <= 2;
end;
",
    );
}
