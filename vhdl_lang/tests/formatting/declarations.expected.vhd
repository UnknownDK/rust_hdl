package p is
    type state_t is (idle, busy);
    type pair_t is record
        left: integer;
        right: integer;
    end record;
    constant zero: integer := 0;
    procedure set_value(
        signal value: out integer
    );
end package;

package body p is
    procedure set_value(
        signal value: out integer
    ) is
        variable next_value: integer;
    begin
        next_value := zero;
        value <= next_value;
    end procedure;
end package body;
