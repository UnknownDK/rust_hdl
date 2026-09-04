package gain_tables is
type GainCodeTable_t is array (0 to 30) of real;
constant GAIN_CODE_TABLE_C: GainCodeTable_t := (-54.0, -50.0, -46.0, -42.0, -39.0, -37.0, -35.0, -33.0, -31.0, -29.0, -27.0, -25.0, -23.0, -21.0, -19.0, -17.0, -15.5, -14.5, -13.5, -12.5, -11.5, -10.5, -9.5, -8.5, -7.75, -7.25, -6.75, -6.25, -5.75, -5.25, 100000.0);
constant COMPLEX_TABLE_C: GainCodeTable_t := (calculate(first_value, second_value, third_value), first_value + second_value, DEFAULT_VALUE, 100000.0);
end;
